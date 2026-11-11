//! Asking a service on this machine over plain HTTP, as the operator's own tools would.

use std::io::{Read as _, Write as _};
use std::net::TcpStream;
use std::time::Duration;

/// How long one call may take.
const PATIENCE: Duration = Duration::from_secs(60);

/// What a service answered: its status and its body.
#[derive(Debug)]
pub struct Answered {
    /// The status code.
    pub status: u16,
    /// The body, as text.
    pub body: String,
}

/// `method` on `path` of the service on loopback at `port`, presenting `key` as an API
/// key and sending `body` as JSON where there is one.
///
/// # Errors
///
/// Where the service cannot be reached or answers nothing HTTP.
pub fn call(
    method: &str,
    port: u16,
    path: &str,
    key: &str,
    body: Option<&str>,
) -> Result<Answered, String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port))
        .map_err(|why| format!("127.0.0.1:{port}: {why}"))?;
    let _ = stream.set_read_timeout(Some(PATIENCE));
    let sent = body.unwrap_or_default();
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nX-Api-Key: {key}\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{sent}",
        sent.len()
    )
    .map_err(|why| format!("{method} {path}: {why}"))?;
    let mut answer = String::new();
    stream
        .read_to_string(&mut answer)
        .map_err(|why| format!("{method} {path}: {why}"))?;
    let (head, rest) = answer.split_once("\r\n\r\n").unwrap_or((&answer, ""));
    let status = head
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .ok_or_else(|| format!("{method} {path}: no status in {head}"))?;
    let chunked = head.lines().any(|line| {
        line.to_ascii_lowercase()
            .starts_with("transfer-encoding: chunked")
    });
    let body = if chunked {
        unchunked(rest)
    } else {
        rest.to_owned()
    };
    Ok(Answered { status, body })
}

/// A chunked body, its chunks joined.
fn unchunked(mut rest: &str) -> String {
    let mut body = String::new();
    while let Some((size, after)) = rest.split_once("\r\n") {
        let Ok(size) = usize::from_str_radix(size.trim(), 16) else {
            break;
        };
        if size == 0 || after.len() < size {
            break;
        }
        body.push_str(&after[..size]);
        rest = after[size..].trim_start_matches("\r\n");
    }
    body
}
