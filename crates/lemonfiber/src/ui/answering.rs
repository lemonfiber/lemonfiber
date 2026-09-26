//! Answering one socket: every connection it accepts, each of them bounded.
//!
//! In place of axum's own `serve`, which lets a caller set neither how long a
//! request's headers may take to arrive nor how many connections are held at once.
//! Without the first, a device on the household network can hold a socket open by
//! sending its headers a byte at a time; without the second, it can hold as many as
//! the process has room for. Either leaves the surface answering nobody else.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use hyper_util::rt::{TokioIo, TokioTimer};
use hyper_util::service::TowerToHyperService;
use tokio::net::TcpListener;
use tokio::sync::{watch, Semaphore};
use tokio::task::JoinSet;

/// The bounds one socket answers under.
#[derive(Debug, Clone, Copy)]
pub(super) struct Limits {
    /// How long a request's headers may take to arrive before the connection is closed.
    pub headers_within: Duration,
    /// How many connections one socket holds at once. One more is closed as it arrives
    /// rather than queued: a queue is a place for the connections that did not fit to
    /// wait, and waiting is what a flood is for.
    pub at_once: usize,
    /// How long a stopping socket gives the connections it holds to finish before they
    /// are dropped. A stream never finishes by itself, and a connection that is slow on
    /// purpose does not either; the surface going away is not something either may
    /// hold up.
    pub let_go: Duration,
}

impl Limits {
    /// The bounds the surface is served under.
    pub(super) const SERVED: Self = Self {
        headers_within: Duration::from_secs(10),
        at_once: 64,
        let_go: Duration::from_secs(5),
    };
}

/// How long to wait after the listener fails to accept, before asking it again.
///
/// A failure to accept is usually the process out of descriptors, and asking again at
/// once would spin on it.
const AFTER_A_FAILED_ACCEPT: Duration = Duration::from_millis(100);

/// Answer on `listener` with `surface` until `stop` says to, then let go.
pub(super) async fn answering(
    listener: TcpListener,
    surface: Router,
    mut stop: watch::Receiver<bool>,
    limits: Limits,
) {
    let room = Arc::new(Semaphore::new(limits.at_once));
    let mut held = JoinSet::new();
    loop {
        let accepted = tokio::select! {
            accepted = listener.accept() => accepted,
            _ = stop.changed() => break,
        };
        let Ok((socket, _)) = accepted else {
            tokio::time::sleep(AFTER_A_FAILED_ACCEPT).await;
            continue;
        };
        let Ok(permit) = Arc::clone(&room).try_acquire_owned() else {
            continue;
        };
        held.spawn(connection(
            socket,
            surface.clone(),
            stop.clone(),
            permit,
            limits.headers_within,
        ));
    }
    drop(listener);
    let _ = tokio::time::timeout(limits.let_go, async {
        while held.join_next().await.is_some() {}
    })
    .await;
    held.abort_all();
}

/// One connection, answered until it ends or the socket it came in on stops.
async fn connection(
    socket: tokio::net::TcpStream,
    surface: Router,
    mut stop: watch::Receiver<bool>,
    _room: tokio::sync::OwnedSemaphorePermit,
    headers_within: Duration,
) {
    let answered = hyper::server::conn::http1::Builder::new()
        .timer(TokioTimer::new())
        .header_read_timeout(headers_within)
        .serve_connection(TokioIo::new(socket), TowerToHyperService::new(surface));
    tokio::pin!(answered);
    tokio::select! {
        _ = answered.as_mut() => {}
        _ = stop.changed() => {
            answered.as_mut().graceful_shutdown();
            let _ = answered.await;
        }
    }
}

#[cfg(test)]
mod tests;
