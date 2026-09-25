//! An engine of our own, answering only what the adapter asks.
//!
//! Deliberately not a general Docker implementation: it serves the handful of routes
//! this adapter uses and 404s the rest, because a fake that grew to cover the whole
//! API would need tests of its own.
//!
//! A module of its own rather than one inside the file that first needed it: the
//! listing, streaming and exec tests all drive it, and two engines meant to answer the
//! same way are two engines that eventually do not.

use std::path::PathBuf;

use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot::{channel, Receiver, Sender};
use tokio::task::JoinHandle;

/// What the engine says when a route is asked for.
#[derive(Debug, Clone)]
pub enum Reply {
    /// A complete body, under a status code.
    Body(u16, String),
    /// Docker's multiplexed stream framing, as logs arrive in.
    Multiplexed(Vec<(u8, String)>),
    /// The same framing, behind the protocol upgrade `exec` performs.
    Upgraded(Vec<(u8, String)>),
    /// An upgrade whose one frame promises more than it delivers.
    ///
    /// What a stream cut mid-chunk looks like from the reading end: a header
    /// saying how much is coming, and then the connection ending before it does.
    /// A real one comes from a container that died or a tunnel that dropped, and
    /// the reading end cannot tell those apart from this.
    Cut(u8, String),
}

/// The API version this engine claims, which is deliberately not the one
/// the adapter was compiled against — so a test can prove the two were
/// reconciled rather than assumed.
pub const CLAIMED_VERSION: &str = "1.44";

/// The head of an upgraded response, which has no length: the body is whatever
/// arrives until the connection closes, which is the point of upgrading.
fn upgraded() -> &'static str {
    "HTTP/1.1 101 UPGRADED\r\nContent-Type: \
     application/vnd.docker.multiplexed-stream\r\n\
     Connection: Upgrade\r\nUpgrade: tcp\r\n\r\n"
}

/// One multiplexed frame: stream number, length, payload.
fn frame(stream: u8, text: &str) -> Vec<u8> {
    let length = u32::try_from(text.len()).unwrap_or_default();
    let mut out = vec![stream, 0, 0, 0];
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(text.as_bytes());
    out
}

/// Everything a reply puts on the wire, headers included.
fn rendered(reply: &Reply) -> Vec<u8> {
    match reply {
        Reply::Body(status, body) => {
            let head = format!(
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let mut out = head.into_bytes();
            out.extend_from_slice(body.as_bytes());
            out
        }
        Reply::Multiplexed(frames) => {
            let body: Vec<u8> = frames
                .iter()
                .flat_map(|(stream, text)| frame(*stream, text))
                .collect();
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/vnd.docker.multiplexed-stream\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let mut out = head.into_bytes();
            out.extend_from_slice(&body);
            out
        }
        // An upgraded response has no length: the body is whatever arrives
        // until the connection closes, which is the point of upgrading.
        Reply::Upgraded(frames) => {
            let mut out = upgraded().as_bytes().to_vec();
            for (stream, text) in frames {
                out.extend_from_slice(&frame(*stream, text));
            }
            out
        }
        Reply::Cut(stream, text) => {
            let mut out = upgraded().as_bytes().to_vec();
            // A header for twice what follows. The reading end waits for the
            // rest, the connection ends, and the chunk never completes.
            let promised = u32::try_from(text.len().saturating_mul(2)).unwrap_or_default();
            out.push(*stream);
            out.extend_from_slice(&[0, 0, 0]);
            out.extend_from_slice(&promised.to_be_bytes());
            out.extend_from_slice(text.as_bytes());
            out
        }
    }
}

/// Read one request, far enough to know what was asked for.
///
/// Bodies are read and discarded rather than ignored: a server that answers
/// before the client has finished sending leaves the client writing into a
/// closed socket, which surfaces as a transport error in a test that was
/// about something else entirely.
async fn request(socket: &mut UnixStream) -> Option<String> {
    let mut received = Vec::new();
    let mut byte = [0_u8; 1];
    while !received.ends_with(b"\r\n\r\n") && socket.read(&mut byte).await.ok()? != 0 {
        received.push(byte[0]);
    }

    // Matched without regard to case, because header names are
    // case-insensitive and the client that will actually call this sends
    // them in lower case. Matching the spelling in the specification
    // instead is a body silently never read.
    let head = String::from_utf8_lossy(&received).into_owned();
    let length: usize = head
        .lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse().ok())
        .unwrap_or_default();
    if length > 0 {
        let mut body = vec![0_u8; length];
        socket.read_exact(&mut body).await.ok()?;
    }

    head.lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .map(ToOwned::to_owned)
}

/// An engine listening on a socket of its own.
pub struct Engine {
    /// Where it is listening.
    pub socket: PathBuf,
    asked: UnboundedReceiver<String>,
    stopping: Option<Sender<()>>,
    serving: Option<JoinHandle<()>>,
}

impl Engine {
    /// Every route this engine was asked for, in order.
    pub fn asked_for(&mut self) -> Vec<String> {
        let mut seen = Vec::new();
        while let Ok(path) = self.asked.try_recv() {
            seen.push(path);
        }
        seen
    }

    /// Stop answering, and wait until it has actually stopped.
    ///
    /// Tests end by calling this rather than by walking away, so a socket
    /// is never still being served while the next test binds its own.
    pub async fn stop(mut self) {
        if let Some(stopping) = self.stopping.take() {
            let _ = stopping.send(());
        }
        if let Some(serving) = self.serving.take() {
            let _ = serving.await;
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket);
    }
}

/// Start an engine answering `routes`, matched as substrings of the path.
///
/// The version route is always present, because the adapter settles the API
/// version before it asks anything else and every test would otherwise have
/// to declare that route itself.
pub fn engine(name: &str, routes: Vec<(&'static str, Reply)>) -> Engine {
    // Not the platform's temporary directory: a Unix socket path is capped
    // near a hundred characters, and macOS puts per-user temporaries deep
    // enough to exceed it.
    let socket = PathBuf::from(format!("/tmp/lf-{}-{name}.sock", std::process::id()));
    let _ = std::fs::remove_file(&socket);

    let version = format!(r#"{{"ApiVersion":"{CLAIMED_VERSION}","Version":"29.4.0"}}"#);
    let mut table: Vec<(String, Reply)> = vec![("/version".to_owned(), Reply::Body(200, version))];
    table.extend(
        routes
            .into_iter()
            .map(|(path, reply)| (path.to_owned(), reply)),
    );

    let (asked, receiver) = unbounded_channel();
    let (stopping, stopped) = channel();
    let serving = UnixListener::bind(&socket)
        .ok()
        .map(|listener| tokio::spawn(answer(listener, table, asked, stopped)));

    Engine {
        socket,
        asked: receiver,
        stopping: Some(stopping),
        serving,
    }
}

/// Answer requests until asked to stop.
async fn answer(
    listener: UnixListener,
    table: Vec<(String, Reply)>,
    asked: UnboundedSender<String>,
    mut stopped: Receiver<()>,
) {
    loop {
        tokio::select! {
            Ok((mut socket, _)) = listener.accept() => {
                let table = table.clone();
                let asked = asked.clone();
                tokio::spawn(async move {
                    if let Some(path) = request(&mut socket).await {
                        let _ = asked.send(path.clone());

                        let found = table
                            .iter()
                            .find(|(route, _)| path.contains(route.as_str()))
                            .map(|(_, reply)| reply.clone());
                        let reply = found.unwrap_or(Reply::Body(
                            404,
                            r#"{"message":"no such route in this engine"}"#.to_owned(),
                        ));

                        let _ = socket.write_all(&rendered(&reply)).await;
                        let _ = socket.flush().await;
                        let _ = socket.shutdown().await;
                    }
                });
            }
            _ = &mut stopped => break,
        }
    }
}
