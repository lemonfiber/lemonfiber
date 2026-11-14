//! The pass service, `lemonfiber pass`: the stream guard asks it whether the pass a
//! request for a live stream's or an item's bytes carries is good, and it answers until
//! it is stopped. An item pass it confirms hands the guard the session it seals, in a
//! header of the answer alone; the service writes neither a pass nor a session anywhere.
//!
//! It reads two files from its configuration directory, both written by the core and
//! mounted read-only: the key and the media server's live-stream and byte paths, read again for
//! every question so a file the core rewrites holds from the next one. It holds nothing
//! else, writes nothing, and answers on one route.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse as _, Response};
use lemonfiber_core::pass::{
    Confirmed, File, Key, Paths, Renewals, Watching, FORWARDED, PATH, SESSION,
};

/// What the service holds while it answers.
struct Held {
    /// The key every pass is made with.
    key: Key,
    /// The media server's live-stream and byte paths, compiled.
    watching: Watching,
    /// Each pass the core renewed.
    renewals: Renewals,
}

/// Answer the guard at `at`, the service's address on the network it shares with the
/// guard, from the files under `config`, for as long as the process runs.
pub(crate) async fn answering(config: &Path, at: std::net::SocketAddr) -> ExitCode {
    serve(config, at, Box::pin(std::future::pending())).await
}

/// The files the service answers from, compiled as their text last read.
struct Compiled {
    /// The key file's text.
    key: String,
    /// The paths file's text.
    paths: String,
    /// The renewals file's text, where the core has written one.
    renewals: Option<String>,
    /// What the files compile to.
    held: Arc<Held>,
}

/// The configuration directory, read for every question and compiled again only where a
/// file's text has changed since the last.
struct Reading {
    /// The directory holding the files.
    config: PathBuf,
    /// The files as last compiled.
    last: Mutex<Option<Compiled>>,
}

impl Reading {
    /// What the files under the directory hold now.
    fn current(&self) -> Result<Arc<Held>, String> {
        let Texts {
            key,
            paths,
            renewals,
        } = texts(&self.config)?;
        let mut last = self
            .last
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(compiled) = last.as_ref().filter(|compiled| {
            compiled.key == key && compiled.paths == paths && compiled.renewals == renewals
        }) {
            return Ok(Arc::clone(&compiled.held));
        }
        let held = Arc::new(compiled(&key, &paths, renewals.as_deref())?);
        *last = Some(Compiled {
            key,
            paths,
            renewals,
            held: Arc::clone(&held),
        });
        Ok(held)
    }
}

/// Answer the guard at `at` from the files under `config` until `stop` resolves.
pub(crate) async fn serve(
    config: &Path,
    at: std::net::SocketAddr,
    stop: std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>,
) -> ExitCode {
    let reading = Reading {
        config: config.to_path_buf(),
        last: Mutex::new(None),
    };
    if let Err(why) = reading.current() {
        crate::say::complained(&format!("the pass service cannot start: {why}"));
        return ExitCode::FAILURE;
    }
    let listener = match tokio::net::TcpListener::bind(at).await {
        Ok(listener) => listener,
        Err(why) => {
            crate::say::complained(&format!("the pass service cannot listen at {at}: {why}"));
            return ExitCode::FAILURE;
        }
    };
    let routed = axum::Router::new()
        .route(PATH, axum::routing::get(answer))
        .with_state(Arc::new(reading));
    axum::serve(listener, routed)
        .with_graceful_shutdown(stop)
        .await
        .map_or(ExitCode::FAILURE, |()| ExitCode::SUCCESS)
}

/// The text of each file under `config`.
struct Texts {
    /// The key file's.
    key: String,
    /// The paths file's.
    paths: String,
    /// The renewals file's, where the core has written one.
    renewals: Option<String>,
}

/// The text of the key file, of the paths file and of the renewals file under `config`,
/// the last absent until the core renews a pass.
fn texts(config: &Path) -> Result<Texts, String> {
    let read = |file: File| {
        std::fs::read_to_string(config.join(file.name()))
            .map_err(|why| format!("{} could not be read: {why}", file.name()))
    };
    let renewals = match std::fs::read_to_string(config.join(File::Renewals.name())) {
        Ok(text) => Some(text),
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => None,
        Err(why) => {
            return Err(format!(
                "{} could not be read: {why}",
                File::Renewals.name()
            ))
        }
    };
    Ok(Texts {
        key: read(File::Key)?,
        paths: read(File::Paths)?,
        renewals,
    })
}

/// The key, the paths and the renewals the texts `key`, `paths` and `renewals` hold.
fn compiled(key: &str, paths: &str, renewals: Option<&str>) -> Result<Held, String> {
    let key = Key::read(key).map_err(|refused| refused.to_string())?;
    let paths = Paths::read(paths).map_err(|refused| refused.to_string())?;
    let watching = Watching::of(&paths)
        .map_err(|pattern| format!("the path {pattern} cannot be matched with"))?;
    let renewals = renewals
        .map(Renewals::read)
        .transpose()
        .map_err(|refused| refused.to_string())?
        .unwrap_or_else(Renewals::none);
    Ok(Held {
        key,
        watching,
        renewals,
    })
}

/// The guard's question, answered at this moment from the files as they are now; a
/// refusal where they cannot be read.
async fn answer(State(reading): State<Arc<Reading>>, headers: HeaderMap) -> Response {
    match reading.current() {
        Ok(held) => answered(&held, &headers, now()),
        Err(why) => {
            crate::say::complained(&format!("the pass service refused a question: {why}"));
            StatusCode::FORBIDDEN.into_response()
        }
    }
}

/// `204` where the request `headers` forward carries a pass good at `now` for the live
/// stream or the item it names, with an item pass's session in [`SESSION`], and `403`
/// for anything else, a request naming neither included.
fn answered(held: &Held, headers: &HeaderMap, now: u64) -> Response {
    let confirmed = headers
        .get(FORWARDED)
        .and_then(|forwarded| forwarded.to_str().ok())
        .and_then(|forwarded| {
            held.watching
                .confirms(&held.key, forwarded, now, &held.renewals)
        });
    match confirmed {
        Some(Confirmed::Live) => StatusCode::NO_CONTENT.into_response(),
        Some(Confirmed::Item(session)) => match HeaderValue::from_str(&session.token) {
            Ok(mut token) => {
                token.set_sensitive(true);
                (StatusCode::NO_CONTENT, [(SESSION, token)]).into_response()
            }
            Err(_) => StatusCode::FORBIDDEN.into_response(),
        },
        None => StatusCode::FORBIDDEN.into_response(),
    }
}

/// This moment, in seconds since the Unix epoch; a clock before the epoch reads as the
/// last moment there is, which no pass outlives.
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(u64::MAX, |since| since.as_secs())
}

#[cfg(test)]
mod tests;
