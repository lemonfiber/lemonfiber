//! What lemonfiber answers itself while it serves: a stand-in on the one address the
//! containers it serves reach this machine by, answering what it was given and refusing
//! everything else.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::pin::Pin;
use std::sync::Arc;

use bollard::Docker;
use futures_util::{Stream, StreamExt as _};
use tokio::io::{AsyncBufReadExt as _, AsyncReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

use lemonfiber_ports::program::Unran;
use lemonfiber_ports::serving::{Answering, StandIn};

use super::program::refused;
use super::Daemon;

/// The most of a request read before it is answered.
const MOST: u64 = 8192;

/// The network whose gateway the engine resolves `host-gateway` to.
const BRIDGE: &str = "bridge";

/// What each engine that forwards `host-gateway` to this machine's own loopback interface
/// names itself.
const LOOPBACK_ENGINES: [&str; 2] = ["Docker Desktop", "OrbStack"];

/// What a stand-in answers anything it was not given.
const REFUSED: &str = r#"{"status":false,"error":"not answered here"}"#;

/// What opens a value a body takes from what the stand-in heard.
const HEARD: &str = "{query:";

/// The value each query parameter held the last time a request carried it.
type Heard = BTreeMap<String, String>;

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
    let heard = Arc::new(Mutex::new(Heard::new()));
    while let Some(taken) = accepted.next().await {
        match taken {
            Ok(stream) => {
                let stand_in = Arc::clone(&stand_in);
                let heard = Arc::clone(&heard);
                tokio::spawn(async move { answered(stream, &stand_in, &heard).await });
            }
            Err(_) => tokio::time::sleep(crate::AFTER_A_FAILED_ACCEPT).await,
        }
    }
}

/// Where the engine resolves `host-gateway` to on this machine: its loopback interface
/// under an engine that forwards `host-gateway` there, and the default bridge network's
/// gateway otherwise.
async fn gateway(docker: &Docker) -> Result<IpAddr, Unran> {
    let info = docker.info().await.map_err(|error| refused(&error))?;
    if info.operating_system.as_deref().is_some_and(|system| {
        LOOPBACK_ENGINES
            .iter()
            .any(|engine| system.contains(engine))
    }) {
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
async fn answered(mut stream: TcpStream, stand_in: &StandIn, heard: &Mutex<Heard>) {
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
    let mut asked = first.split(' ');
    let method = asked.next().unwrap_or_default();
    let target = asked.next().unwrap_or_default();
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let response = {
        let mut heard = heard.lock().await;
        heard.extend(pairs(query).map(|(name, value)| (name.to_owned(), value.to_owned())));
        response(stand_in, method, path, query, &heard)
    };
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}

/// The whole answer `stand_in` gives `method` at `path` asked with `query`, its headers and
/// body included, each value a body takes from `heard` filled in.
fn response(stand_in: &StandIn, method: &str, path: &str, query: &str, heard: &Heard) -> String {
    let (status, body) = answer(stand_in, method, path, query);
    let body = body.map(|body| filled(body, heard));
    let mut response = format!("HTTP/1.1 {status} \r\n");
    for (name, value) in &stand_in.headers {
        let _ = write!(response, "{name}: {value}\r\n");
    }
    if let Some(body) = &body {
        let _ = write!(
            response,
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        );
    }
    response.push_str("Connection: close\r\n\r\n");
    response.push_str(&body.unwrap_or_default());
    response
}

/// Each query parameter in `query` that has a value, as its name and that value.
fn pairs(query: &str) -> impl Iterator<Item = (&str, &str)> {
    query.split('&').filter_map(|pair| pair.split_once('='))
}

/// `body` with each `{query:<name>}` in it the value `heard` holds for `<name>`, written as
/// JSON string content, and nothing where it holds none.
fn filled(body: &str, heard: &Heard) -> String {
    let mut written = String::new();
    let mut rest = body;
    while let Some((before, after)) = rest.split_once(HEARD) {
        written.push_str(before);
        let Some((name, tail)) = after.split_once('}') else {
            written.push_str(HEARD);
            rest = after;
            break;
        };
        let value = heard.get(name).map(String::as_str).unwrap_or_default();
        let quoted = serde_json::Value::from(value).to_string();
        let content = quoted
            .strip_prefix('"')
            .and_then(|open| open.strip_suffix('"'));
        written.push_str(content.unwrap_or_default());
        rest = tail;
    }
    written.push_str(rest);
    written
}

/// The status and body `stand_in` answers `method` at `path` asked with `query` with.
fn answer<'a>(
    stand_in: &'a StandIn,
    method: &str,
    path: &str,
    query: &str,
) -> (u16, Option<&'a str>) {
    let held = |name: &str, value: &str| pairs(query).any(|pair| pair == (name, value));
    stand_in
        .answers
        .iter()
        .find(|said| {
            said.method == method
                && said.path.eq_ignore_ascii_case(path)
                && said
                    .query
                    .as_ref()
                    .is_none_or(|(name, value)| held(name, value))
        })
        .map_or((404, Some(REFUSED)), |said| {
            (said.status, said.body.as_deref())
        })
}

#[cfg(test)]
mod tests;
