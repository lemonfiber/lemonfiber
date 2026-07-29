//! The HTTP adapter, driven against a fake server that speaks raw HTTP.
//!
//! The adapter's whole job is to turn a request into a real HTTP call and the
//! reply into a response, so what is replaced here is the service: a socket that
//! answers with exactly the bytes a test wants. That exercises the request
//! building, the header and body handling, status and body decoding, and the
//! unreachable path — the whole adapter, with no service running.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use lemonfiber_core::adapters::Web;
use lemonfiber_core::ports::http::{Http, Method, Request};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// What the fake server sends back.
enum Reply {
    /// A complete response with this status and body.
    Whole(u16, &'static str),
    /// A response that promises more body than it sends, then closes — so the
    /// client cannot read the body it was told to expect.
    Truncated,
}

/// A running fake server on localhost, and the request it captured.
struct Server {
    base: String,
    captured: Arc<Mutex<String>>,
    handle: tokio::task::JoinHandle<()>,
}

impl Server {
    /// The raw request the server received.
    fn request(&self) -> String {
        self.captured
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    /// Stop serving; tests end by calling this rather than by walking away.
    async fn stop(self) {
        self.handle.abort();
        let _ = self.handle.await;
    }
}

/// Bind a localhost server that answers one request with `reply`.
async fn serve(reply: Reply) -> Server {
    let captured = Arc::new(Mutex::new(String::new()));
    let listener = TcpListener::bind("127.0.0.1:0").await;
    match listener {
        Ok(listener) => {
            let port = listener.local_addr().map_or(0, |address| address.port());
            let recorder = Arc::clone(&captured);
            let handle = tokio::spawn(answer(listener, reply, recorder));
            Server {
                base: format!("http://127.0.0.1:{port}"),
                captured,
                handle,
            }
        }
        Err(_) => Server {
            base: "http://127.0.0.1:0".to_owned(),
            captured,
            handle: tokio::spawn(async {}),
        },
    }
}

/// Accept one connection, record the whole request, then send the reply.
async fn answer(listener: TcpListener, reply: Reply, captured: Arc<Mutex<String>>) {
    let Ok((mut socket, _)) = listener.accept().await else {
        return;
    };

    // Read until the client stops sending and waits for a reply: a short quiet
    // period is the signal the whole request — headers and any body — has arrived.
    let mut received = Vec::new();
    let mut buffer = vec![0u8; 8192];
    loop {
        match tokio::time::timeout(Duration::from_millis(250), socket.read(&mut buffer)).await {
            Ok(Ok(0) | Err(_)) | Err(_) => break,
            Ok(Ok(read)) => received.extend_from_slice(buffer.get(..read).unwrap_or_default()),
        }
    }
    if let (Ok(text), Ok(mut guard)) = (std::str::from_utf8(&received), captured.lock()) {
        text.clone_into(&mut guard);
    }

    let bytes = match reply {
        Reply::Whole(status, body) => {
            format!(
                "HTTP/1.1 {status} X\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            )
        }
        // Promise a hundred bytes, send five, then close.
        Reply::Truncated => "HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nshort".to_owned(),
    };
    let _ = socket.write_all(bytes.as_bytes()).await;
    let _ = socket.flush().await;
    let _ = socket.shutdown().await;
}

/// A URL on a port nothing is listening on: bound to learn a free port, then
/// dropped so a connection to it is refused.
async fn dead_url() -> String {
    let port = TcpListener::bind("127.0.0.1:0")
        .await
        .ok()
        .and_then(|listener| listener.local_addr().ok())
        .map_or(0, |address| address.port());
    format!("http://127.0.0.1:{port}")
}

#[tokio::test]
async fn a_get_returns_the_status_and_the_body() {
    let server = serve(Reply::Whole(200, "who am i")).await;
    let request = Request {
        method: Method::Get,
        url: format!("{}/api/v3/system/status", server.base),
        headers: Vec::new(),
        body: None,
    };
    let response = Web::default().send(&request).await;
    let sent = server.request();
    server.stop().await;

    let response = response.ok();
    assert_eq!(
        response.as_ref().map(|answer| answer.status),
        Some(200),
        "the status is read back"
    );
    assert_eq!(
        response.as_ref().map(|answer| answer.body.as_str()),
        Some("who am i")
    );
    assert!(
        sent.starts_with("GET "),
        "a GET was actually sent: {sent:?}"
    );
}

#[tokio::test]
async fn a_post_carries_its_headers_and_body() {
    let server = serve(Reply::Whole(201, "")).await;
    let request = Request {
        method: Method::Post,
        url: format!("{}/api/v3/rootfolder", server.base),
        headers: vec![("X-Api-Key".to_owned(), "the-secret".to_owned())],
        body: Some(r#"{"path":"/data/media/tv"}"#.to_owned()),
    };
    let response = Web::new().send(&request).await;
    let sent = server.request();
    server.stop().await;

    assert_eq!(response.ok().map(|answer| answer.status), Some(201));
    assert!(sent.starts_with("POST "), "a POST was sent: {sent:?}");
    assert!(sent.contains("x-api-key: the-secret") || sent.contains("X-Api-Key: the-secret"));
    assert!(
        sent.contains(r#"{"path":"/data/media/tv"}"#),
        "the body was sent"
    );
}

#[tokio::test]
async fn a_refusal_is_a_response_rather_than_a_failure() {
    // A 401 is an answer: the credential was rejected, but the service was
    // reached. That distinction is the whole point of the port.
    let server = serve(Reply::Whole(401, "unauthorized")).await;
    let request = Request {
        method: Method::Get,
        url: format!("{}/api/v3/system/status", server.base),
        headers: Vec::new(),
        body: None,
    };
    let response = Web::new().send(&request).await;
    server.stop().await;

    let response = response.ok();
    assert_eq!(response.as_ref().map(|answer| answer.status), Some(401));
    assert_eq!(response.map(|answer| answer.is_success()), Some(false));
}

#[tokio::test]
async fn a_service_that_is_not_listening_is_unreachable() {
    let request = Request {
        method: Method::Get,
        url: format!("{}/api/v3/system/status", dead_url().await),
        headers: Vec::new(),
        body: None,
    };
    let outcome = Web::new().send(&request).await;
    assert!(
        outcome.is_err(),
        "nothing answered, so it is unreachable rather than any status"
    );
}

#[tokio::test]
async fn a_secret_in_the_query_is_masked_out_of_an_unreachable() {
    // An indexer authenticates by an apikey in the URL, not a header. When it
    // cannot be reached the failure must not carry that key — not in the URL it
    // keeps, and not in the reason read from the transport error, which can quote
    // the URL it failed on.
    let request = Request {
        method: Method::Get,
        url: format!("{}/api?t=search&apikey=super-secret-key", dead_url().await),
        headers: Vec::new(),
        body: None,
    };
    let outcome = Web::new().send(&request).await;
    let unreachable = outcome.err();
    assert!(
        unreachable.is_some(),
        "nothing answered, so it is unreachable"
    );
    let rendered = unreachable
        .map(|failure| format!("{} {}", failure.url, failure.reason))
        .unwrap_or_default();
    assert!(
        !rendered.contains("super-secret-key"),
        "the apikey must appear nowhere the operator could see it: {rendered:?}"
    );
    assert!(
        rendered.contains("t=search"),
        "a non-secret parameter is left legible for diagnosis: {rendered:?}"
    );
}

#[tokio::test]
async fn a_body_that_does_not_arrive_is_unreachable() {
    // The server promises a body and closes without sending it. A truncated
    // answer is no answer: the port reports one response or none.
    let server = serve(Reply::Truncated).await;
    let request = Request {
        method: Method::Get,
        url: format!("{}/api/v3/system/status", server.base),
        headers: Vec::new(),
        body: None,
    };
    let outcome = Web::new().send(&request).await;
    server.stop().await;
    assert!(outcome.is_err(), "a body that never arrives is unreachable");
}
