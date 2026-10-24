//! What lemonfiber answers itself while it serves: a stand-in on the one address the
//! containers it serves reach this machine by, answering what it was given and refusing
//! everything else.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use bollard::Docker;
use futures_util::{Stream, StreamExt as _};
use tokio::io::{AsyncBufReadExt as _, AsyncReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::net::{TcpListener, TcpStream};

use lemonfiber_ports::program::Unran;
use lemonfiber_ports::serving::{Answering, StandIn};

use super::program::refused;
use super::Daemon;

/// The most of a request read before it is answered.
const MOST: u64 = 8192;

/// The network whose gateway the engine resolves `host-gateway` to.
const BRIDGE: &str = "bridge";

/// What the engine names itself under Docker Desktop, which forwards `host-gateway` to this
/// machine's own loopback interface.
const DESKTOP: &str = "Docker Desktop";

/// What a stand-in answers anything it was not given.
const REFUSED: &str = r#"{"status":false,"error":"not answered here"}"#;

/// How long a stand-in waits after a connection it could not take before the next.
const AFTER_A_FAILED_ACCEPT: Duration = Duration::from_millis(100);

/// Each connection a stand-in is asked on, as it is taken.
type Accepted = Pin<Box<dyn Stream<Item = std::io::Result<TcpStream>> + Send>>;

/// The task answering a stand-in, stopped when it is dropped.
struct Answered(tokio::task::JoinHandle<()>);

impl Answering for Answered {}

impl Drop for Answered {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Answer `stand_in` at the address the engine's containers reach this machine by.
pub(super) async fn stood_in(
    daemon: &Daemon,
    stand_in: &StandIn,
) -> Result<Box<dyn Answering>, Unran> {
    let docker = daemon
        .client()
        .await
        .map_err(|failure| Unran::Unreachable {
            reason: failure.to_string(),
        })?;
    let at = SocketAddr::new(gateway(docker).await?, stand_in.port);
    let listener = TcpListener::bind(at)
        .await
        .map_err(|error| Unran::Refused {
            reason: format!("{at} could not be taken to answer on: {error}"),
        })?;
    let accepted = futures_util::stream::unfold(listener, |listener| async move {
        let taken = listener.accept().await.map(|(stream, _)| stream);
        Some((taken, listener))
    });
    Ok(Box::new(Answered(tokio::spawn(answering(
        Box::pin(accepted),
        Arc::new(stand_in.clone()),
    )))))
}

/// Answer each connection `accepted` yields as `stand_in` says, pausing after one that
/// could not be taken.
async fn answering(mut accepted: Accepted, stand_in: Arc<StandIn>) {
    while let Some(taken) = accepted.next().await {
        match taken {
            Ok(stream) => {
                let stand_in = Arc::clone(&stand_in);
                tokio::spawn(async move { answered(stream, &stand_in).await });
            }
            Err(_) => tokio::time::sleep(AFTER_A_FAILED_ACCEPT).await,
        }
    }
}

/// Where the engine resolves `host-gateway` to on this machine: its loopback interface
/// under Docker Desktop, and the default bridge network's gateway otherwise.
async fn gateway(docker: &Docker) -> Result<IpAddr, Unran> {
    let info = docker.info().await.map_err(|error| refused(&error))?;
    if info
        .operating_system
        .as_deref()
        .is_some_and(|system| system.contains(DESKTOP))
    {
        return Ok(IpAddr::V4(Ipv4Addr::LOCALHOST));
    }
    let bridge = docker
        .inspect_network(BRIDGE, None)
        .await
        .map_err(|error| refused(&error))?;
    bridge
        .ipam
        .and_then(|ipam| ipam.config)
        .into_iter()
        .flatten()
        .find_map(|config| config.gateway.and_then(|gateway| gateway.parse().ok()))
        .ok_or_else(|| Unran::Refused {
            reason: "the engine's bridge network names no gateway to answer on".to_owned(),
        })
}

/// Read one request from `stream`, its headers to their end, and answer it as `stand_in`
/// says, closing after; what cannot be read is answered as far as it was read, and a
/// connection that cannot be written to is left.
async fn answered(mut stream: TcpStream, stand_in: &StandIn) {
    let mut reading = BufReader::new((&mut stream).take(MOST));
    let mut first = Vec::new();
    let mut header = Vec::new();
    let _ = reading.read_until(b'\n', &mut first).await;
    while reading
        .read_until(b'\n', &mut header)
        .await
        .is_ok_and(|read| read > 0)
        && !header.trim_ascii().is_empty()
    {
        header.clear();
    }
    let first = String::from_utf8_lossy(&first);
    let target = first.split(' ').nth(1).unwrap_or_default();
    let (status, body) = answer(stand_in, target);
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

/// The status and body `stand_in` answers `target` with.
fn answer<'a>(stand_in: &'a StandIn, target: &str) -> (&'static str, &'a str) {
    let asked = target.split_once('?').and_then(|(_, query)| {
        query.split('&').find_map(|pair| {
            let (name, value) = pair.split_once('=')?;
            (name == stand_in.asked_by).then_some(value)
        })
    });
    stand_in
        .answers
        .iter()
        .find(|(named, _)| Some(named.as_str()) == asked)
        .map_or(("404 Not Found", REFUSED), |(_, said)| ("200 OK", said))
}

#[cfg(test)]
mod tests;
