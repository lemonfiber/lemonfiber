//! The pass service, `lemonfiber pass`: the stream guard asks it whether the pass a
//! request for a live stream's bytes carries is good, and it answers until it is stopped.
//!
//! It reads two files from its configuration directory, both written by the core and
//! mounted read-only: the key and the media server's live-stream paths. It holds nothing
//! else, writes nothing, and answers on one route.

use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;

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

/// Answer the guard at `at` from the files under `config` until `stop` resolves.
pub(crate) async fn serve(
    config: &Path,
    at: std::net::SocketAddr,
    stop: std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>,
) -> ExitCode {
    let held = match held(config) {
        Ok(held) => held,
        Err(why) => {
            crate::say::complained(&format!("the pass service cannot start: {why}"));
            return ExitCode::FAILURE;
        }
    };
    let listener = match tokio::net::TcpListener::bind(at).await {
        Ok(listener) => listener,
        Err(why) => {
            crate::say::complained(&format!("the pass service cannot listen at {at}: {why}"));
            return ExitCode::FAILURE;
        }
    };
    let routed = axum::Router::new()
        .route(PATH, axum::routing::get(answer))
        .with_state(Arc::new(held));
    axum::serve(listener, routed)
        .with_graceful_shutdown(stop)
        .await
        .map_or(ExitCode::FAILURE, |()| ExitCode::SUCCESS)
}

/// The key and the live-stream paths under `config`.
fn held(config: &Path) -> Result<Held, String> {
    let read = |file: File| {
        std::fs::read_to_string(config.join(file.name()))
            .map_err(|why| format!("{} could not be read: {why}", file.name()))
    };
    let key = Key::read(&read(File::Key)?).map_err(|refused| refused.to_string())?;
    let live = Live::read(&read(File::Live)?).map_err(|refused| refused.to_string())?;
    let watching = Watching::of(&live)
        .map_err(|pattern| format!("the live-stream path {pattern} cannot be matched with"))?;
    Ok(Held { key, watching })
}

/// `204` where the forwarded request's pass is good for the live stream its path names,
/// and `403` for anything else, a request naming none included.
async fn answer(State(held): State<Arc<Held>>, headers: HeaderMap) -> StatusCode {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(u64::MAX, |since| since.as_secs());
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

#[cfg(test)]
mod tests;
