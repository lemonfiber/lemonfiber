//! The pass service, `lemonfiber pass`: the stream guard asks it whether the pass a
//! request for a live stream's bytes carries is good, and it answers until it is stopped.
//!
//! It reads two files from its configuration directory, both written by the core and
//! mounted read-only: the key and the media server's live-stream paths, read again for
//! every question so a file the core rewrites holds from the next one. It holds nothing
//! else, writes nothing, and answers on one route.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use lemonfiber_core::pass::{File, Key, Live, Watching, FORWARDED, PATH};

/// What the service holds while it answers.
struct Held {
    /// The key every pass is made with.
    key: Key,
    /// The media server's live-stream paths, compiled.
    watching: Watching,
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
    /// The live-stream paths file's text.
    live: String,
    /// What the two compile to.
    held: Arc<Held>,
}

/// The configuration directory, read for every question and compiled again only where a
/// file's text has changed since the last.
struct Reading {
    /// The directory holding the two files.
    config: PathBuf,
    /// The files as last compiled.
    last: Mutex<Option<Compiled>>,
}

impl Reading {
    /// What the files under the directory hold now.
    fn current(&self) -> Result<Arc<Held>, String> {
        let (key, live) = texts(&self.config)?;
        let mut last = self
            .last
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(compiled) = last
            .as_ref()
            .filter(|compiled| compiled.key == key && compiled.live == live)
        {
            return Ok(Arc::clone(&compiled.held));
        }
        let held = Arc::new(compiled(&key, &live)?);
        *last = Some(Compiled {
            key,
            live,
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

/// The text of the key file and of the live-stream paths file under `config`.
fn texts(config: &Path) -> Result<(String, String), String> {
    let read = |file: File| {
        std::fs::read_to_string(config.join(file.name()))
            .map_err(|why| format!("{} could not be read: {why}", file.name()))
    };
    Ok((read(File::Key)?, read(File::Live)?))
}

/// The key and the live-stream paths the texts `key` and `live` hold.
fn compiled(key: &str, live: &str) -> Result<Held, String> {
    let key = Key::read(key).map_err(|refused| refused.to_string())?;
    let live = Live::read(live).map_err(|refused| refused.to_string())?;
    let watching = Watching::of(&live)
        .map_err(|pattern| format!("the live-stream path {pattern} cannot be matched with"))?;
    Ok(Held { key, watching })
}

/// The guard's question, answered at this moment from the files as they are now; a
/// refusal where they cannot be read.
async fn answer(State(reading): State<Arc<Reading>>, headers: HeaderMap) -> StatusCode {
    match reading.current() {
        Ok(held) => answered(&held, &headers, now()),
        Err(why) => {
            crate::say::complained(&format!("the pass service refused a question: {why}"));
            StatusCode::FORBIDDEN
        }
    }
}

/// `204` where the request `headers` forward carries a pass good at `now` for the live
/// stream its path names, and `403` for anything else, a request naming none included.
fn answered(held: &Held, headers: &HeaderMap, now: u64) -> StatusCode {
    let confirmed = headers
        .get(FORWARDED)
        .and_then(|forwarded| forwarded.to_str().ok())
        .is_some_and(|forwarded| held.watching.confirms(&held.key, forwarded, now));
    if confirmed {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::FORBIDDEN
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
