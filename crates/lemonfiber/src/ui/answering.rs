//! Answering one socket: every connection it accepts, each of them bounded.
//!
//! In place of axum's own `serve`, which lets a caller set neither how long a
//! request may take to arrive nor how many connections are held at once. Without
//! the first, a device on the household network can hold a socket open by sending
//! its headers, or the body it promised, a byte at a time; without the second, it
//! can hold as many as the process has room for. Either leaves the surface
//! answering nobody else.

mod deadline;
mod room;

use std::net::SocketAddr;
use std::time::Duration;

use axum::extract::{ConnectInfo, Request};
use axum::Router;
use hyper::body::Incoming;
use hyper_util::rt::{TokioIo, TokioTimer};
use hyper_util::service::TowerToHyperService;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio::task::JoinSet;
use tokio_rustls::TlsAcceptor;
use tower::ServiceExt as _;

use deadline::Deadlined;
use room::{Room, Shares, Slot};

/// The bounds one socket answers under.
#[derive(Debug, Clone, Copy)]
pub(super) struct Limits {
    /// How long a request's headers may take to arrive before the connection is closed.
    pub headers_within: Duration,
    /// How long a request's body may take to arrive once its headers have.
    pub body_within: Duration,
    /// How many connections one socket holds at once, and how they are shared out. One
    /// more is closed as it arrives rather than queued: a queue is a place for the
    /// connections that did not fit to wait, and waiting is what a flood is for.
    pub shares: Shares,
    /// How long a stopping socket gives the connections it holds to finish before they
    /// are dropped. A stream never finishes by itself, and a connection that is slow on
    /// purpose does not either; the surface going away is not something either may
    /// hold up.
    pub let_go: Duration,
}

impl Limits {
    /// The bounds the surface is served under.
    ///
    /// Sixteen connections an address is more than a browser opens to one server, with
    /// room for a second tab and the app beside it; eight are kept for this machine.
    pub(super) const SERVED: Self = Self {
        headers_within: Duration::from_secs(10),
        body_within: Duration::from_secs(10),
        shares: Shares {
            at_once: 64,
            each_peer: 16,
            kept_for_here: 8,
        },
        let_go: Duration::from_secs(5),
    };
}

/// How long to wait after the listener fails to accept, before asking it again.
///
/// A failure to accept is usually the process out of descriptors, and asking again at
/// once would spin on it.
const AFTER_A_FAILED_ACCEPT: Duration = Duration::from_millis(100);

/// Where connections arrive from.
///
/// A trait object rather than the listener itself so a test can stand in a listener
/// that fails to accept, which a real one does when the process is out of
/// descriptors.
#[async_trait::async_trait]
pub(super) trait Accepting: Send {
    /// The next connection, or why none could be taken.
    async fn accept(&self) -> std::io::Result<(TcpStream, SocketAddr)>;
}

#[async_trait::async_trait]
impl Accepting for TcpListener {
    async fn accept(&self) -> std::io::Result<(TcpStream, SocketAddr)> {
        TcpListener::accept(self).await
    }
}

/// Answer on `listener` with `surface` until `stop` says to, then let go.
///
/// Each connection is handed to `tls` first where this run encrypts, inside its own
/// task and within the time its headers are given: a handshake is the other thing a
/// device can send a byte at a time, and one doing so holds its own slot and nobody
/// else's.
pub(super) async fn answering(
    listener: Box<dyn Accepting>,
    surface: Router,
    mut stop: watch::Receiver<bool>,
    limits: Limits,
    tls: Option<TlsAcceptor>,
) {
    let room = Room::sharing(limits.shares);
    let mut held = JoinSet::new();
    loop {
        let accepted = tokio::select! {
            accepted = listener.accept() => accepted,
            _ = stop.changed() => break,
        };
        let Ok((socket, peer)) = accepted else {
            tokio::time::sleep(AFTER_A_FAILED_ACCEPT).await;
            continue;
        };
        let Some(slot) = room.taken(peer.ip()) else {
            continue;
        };
        held.spawn(connection(
            Accepted { socket, peer, slot },
            surface.clone(),
            stop.clone(),
            limits,
            tls.clone(),
        ));
    }
    drop(listener);
    let _ = tokio::time::timeout(limits.let_go, async {
        while held.join_next().await.is_some() {}
    })
    .await;
    held.abort_all();
}

/// What a connection is read from and written to, whether or not it is encrypted.
///
/// A trait object rather than a generic, so the one function below is the one that is
/// run either way — two copies of it would be one that only an encrypted test reaches.
trait Io: AsyncRead + AsyncWrite + Unpin + Send {}

impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}

/// One connection as it was accepted: the socket, who it is from, and the slot it holds.
struct Accepted {
    /// What it is read from and written to.
    socket: TcpStream,
    /// Where it came from, which every request on it carries to the routes.
    peer: SocketAddr,
    /// Its share of the room, given back when the connection ends.
    slot: Slot,
}

/// One connection, answered until it ends or the socket it came in on stops.
///
/// A handshake that fails or does not finish in time ends the connection unanswered:
/// there is nobody on the other end this surface could say anything useful to.
async fn connection(
    accepted: Accepted,
    surface: Router,
    mut stop: watch::Receiver<bool>,
    limits: Limits,
    tls: Option<TlsAcceptor>,
) {
    let Accepted {
        socket,
        peer,
        slot: _held,
    } = accepted;
    let io: Box<dyn Io> = match tls {
        None => Box::new(socket),
        Some(acceptor) => {
            match tokio::time::timeout(limits.headers_within, acceptor.accept(socket)).await {
                Ok(Ok(encrypted)) => Box::new(encrypted),
                Ok(Err(_)) | Err(_) => return,
            }
        }
    };
    let body_within = limits.body_within;
    let surface = surface.map_request(move |request: Request<Incoming>| {
        let mut request = request.map(|body| Deadlined::within(body, body_within));
        request.extensions_mut().insert(ConnectInfo(peer));
        request
    });
    let answered = hyper::server::conn::http1::Builder::new()
        .timer(TokioTimer::new())
        .header_read_timeout(limits.headers_within)
        .serve_connection(TokioIo::new(io), TowerToHyperService::new(surface));
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
