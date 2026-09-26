use std::time::Duration;

use axum::routing::get;
use axum::Router;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;

use super::{answering, Limits};

/// Tight bounds, so each case below takes a fraction of a second.
const TIGHT: Limits = Limits {
    headers_within: Duration::from_millis(300),
    at_once: 2,
    let_go: Duration::from_millis(300),
};

/// A surface with one route that answers and one that never finishes.
fn surface() -> Router {
    Router::new()
        .route("/", get(|| async { "answered" }))
        .route(
            "/forever",
            get(|| async { std::future::pending::<&'static str>().await }),
        )
}

/// A socket being answered, and the switch that stops it.
async fn serving(
    limits: Limits,
) -> Option<(
    std::net::SocketAddr,
    watch::Sender<bool>,
    tokio::task::JoinHandle<()>,
)> {
    let listener = TcpListener::bind("127.0.0.1:0").await.ok()?;
    let at = listener.local_addr().ok()?;
    let (stop, stopped) = watch::channel(false);
    let running = tokio::spawn(answering(listener, surface(), stopped, limits));
    Some((at, stop, running))
}

/// Whether the server closes this connection within `within`.
async fn closed(stream: &mut TcpStream, within: Duration) -> bool {
    let mut buffer = [0_u8; 256];
    loop {
        match tokio::time::timeout(within, stream.read(&mut buffer)).await {
            Ok(Ok(0) | Err(_)) => return true,
            Ok(Ok(_)) => {}
            Err(_) => return false,
        }
    }
}

#[tokio::test]
async fn a_request_is_answered() {
    let Some((at, _stop, _running)) = serving(TIGHT).await else {
        unreachable!("a loopback socket could not be bound");
    };
    let mut stream = TcpStream::connect(at).await.ok();
    let asked = match stream.as_mut() {
        Some(stream) => stream
            .write_all(b"GET / HTTP/1.1\r\nhost: here\r\nconnection: close\r\n\r\n")
            .await
            .is_ok(),
        None => false,
    };
    assert!(asked);
    let mut answer = String::new();
    if let Some(stream) = stream.as_mut() {
        let _ = stream.read_to_string(&mut answer).await;
    }
    assert!(answer.contains("answered"), "{answer}");
}

/// Headers sent a byte at a time, never finished, are cut off rather than held.
#[tokio::test]
async fn headers_that_never_finish_arriving_are_cut_off() {
    let Some((at, _stop, _running)) = serving(TIGHT).await else {
        unreachable!("a loopback socket could not be bound");
    };
    let Ok(mut stream) = TcpStream::connect(at).await else {
        unreachable!("the socket did not accept");
    };
    assert!(stream.write_all(b"GET / HTTP/1.1\r\nhost").await.is_ok());
    assert!(
        closed(&mut stream, Duration::from_secs(3)).await,
        "a slow request held its socket"
    );
}

/// A connection past the ceiling is closed as it arrives.
#[tokio::test]
async fn a_connection_past_the_ceiling_is_closed() {
    let limits = Limits {
        headers_within: Duration::from_secs(10),
        ..TIGHT
    };
    let Some((at, _stop, _running)) = serving(limits).await else {
        unreachable!("a loopback socket could not be bound");
    };
    let mut holding = Vec::new();
    for _ in 0..limits.at_once {
        holding.push(TcpStream::connect(at).await);
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    let Ok(mut one_more) = TcpStream::connect(at).await else {
        unreachable!("the socket did not accept");
    };
    assert!(
        closed(&mut one_more, Duration::from_secs(2)).await,
        "a connection past the ceiling was held"
    );
    assert!(holding.iter().all(Result::is_ok));
}

/// Stopping lets go within its bound, whatever a connection is still doing.
#[tokio::test]
async fn stopping_lets_go_of_a_response_that_never_ends() {
    let Some((at, stop, running)) = serving(TIGHT).await else {
        unreachable!("a loopback socket could not be bound");
    };
    let Ok(mut stream) = TcpStream::connect(at).await else {
        unreachable!("the socket did not accept");
    };
    assert!(stream
        .write_all(b"GET /forever HTTP/1.1\r\nhost: here\r\n\r\n")
        .await
        .is_ok());
    tokio::time::sleep(Duration::from_millis(100)).await;
    let _ = stop.send(true);
    assert!(
        tokio::time::timeout(Duration::from_secs(3), running)
            .await
            .is_ok(),
        "the socket was still held after it was told to stop"
    );
}
