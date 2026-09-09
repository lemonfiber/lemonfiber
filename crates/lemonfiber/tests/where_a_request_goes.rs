//! Where a request actually goes, asked of the transport rather than of the source.
//!
//! `nothing_reports_on_you.rs` holds the closed list of hosts this product may
//! reach, and it holds it by reading source text: every quoted string in the
//! shipped half goes through a host parser, and one that is not written down fails.
//! That is the right shape for the question it asks — *which addresses does this
//! program name* — and it is structurally blind to the one that matters just as
//! much: **which address does a request arrive at**. A host that never appears in
//! the source cannot be swept for, and a service answering `302 Location: …` names
//! one at runtime.
//!
//! So this asks the transport. Two servers on loopback, the first one answering
//! every request with a redirect to the second, and the claim is that the second is
//! never touched: not by the request, not by the credential in its header, not by
//! the credential in its query. The list of hosts is only closed if the host a
//! connection opens to is the host that was asked for, and that is a property of
//! the client rather than of the text.
//!
//! **Why one transport is the whole claim.** This drives `Web`, which is the only
//! `Http` the workspace ships; `architecture.rs` confines `reqwest` to the single
//! adapter it lives in, so a second client built without this policy cannot exist
//! without failing there first. The two together are the whole of it — this one
//! alone would say nothing about a client somebody added elsewhere.
//!
//! **What is not covered.** A host reached because DNS answered with somebody
//! else's address, or because a proxy was interposed, is outside anything a test
//! can see from here. What is closed is the hop a service can ask for on its own
//! account, which is the one it can take without any position on the network.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lemonfiber_core::adapters::Web;
use lemonfiber_core::ports::http::{Http, Method, Request};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// The credential a \*arr request carries, and the one a redirect strips from
/// nothing: reqwest drops `Authorization`, `Cookie` and `Proxy-Authorization` on a
/// cross-host hop and has no reason to know this header is one too.
const API_KEY_HEADER: &str = "X-Api-Key";

/// Assembled rather than written out, and a placeholder rather than a plausible
/// key: a run that reads as a real one in this source is a secret scanner's finding
/// for as long as the commit exists.
fn credential() -> String {
    ["the", "service", "key"].join("-")
}

/// A server on loopback: what it answers, and what reached it.
struct Server {
    base: String,
    /// Every byte of every request that arrived, so an assertion can look for a
    /// credential without knowing which line it would land on.
    heard: Arc<Mutex<Vec<String>>>,
    /// How many connections were accepted, which is the count that matters for a
    /// host that should have been reached zero times — a connection opened and
    /// then abandoned is still a host that learned this machine is here.
    reached: Arc<AtomicUsize>,
    handle: tokio::task::JoinHandle<()>,
}

impl Server {
    /// How many requests arrived.
    fn reached(&self) -> usize {
        self.reached.load(Ordering::SeqCst)
    }

    /// Everything that arrived, as one string to search.
    fn heard(&self) -> String {
        self.heard
            .lock()
            .map(|guard| guard.join("\n"))
            .unwrap_or_default()
    }

    /// Stop serving; tests end by calling this rather than by walking away.
    async fn stop(self) {
        self.handle.abort();
        let _ = self.handle.await;
    }
}

/// Bind a server on loopback answering every request with `reply`.
async fn serve(reply: String) -> Server {
    let heard = Arc::new(Mutex::new(Vec::new()));
    let reached = Arc::new(AtomicUsize::new(0));
    let Ok(listener) = TcpListener::bind("127.0.0.1:0").await else {
        unreachable!("a loopback port is bindable, or no test in this file can run");
    };
    let port = listener.local_addr().map_or(0, |address| address.port());
    let handle = tokio::spawn(answer(
        listener,
        reply,
        Arc::clone(&heard),
        Arc::clone(&reached),
    ));
    Server {
        base: format!("http://127.0.0.1:{port}"),
        heard,
        reached,
        handle,
    }
}

/// Accept connections for as long as the test runs, recording each and replying.
async fn answer(
    listener: TcpListener,
    reply: String,
    heard: Arc<Mutex<Vec<String>>>,
    reached: Arc<AtomicUsize>,
) {
    loop {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        reached.fetch_add(1, Ordering::SeqCst);

        // Read until the client stops sending and waits for a reply: a short quiet
        // period is the signal the whole request — headers and any body — arrived.
        let mut received = Vec::new();
        let mut buffer = vec![0u8; 8192];
        loop {
            match tokio::time::timeout(Duration::from_millis(250), socket.read(&mut buffer)).await {
                Ok(Ok(0) | Err(_)) | Err(_) => break,
                Ok(Ok(read)) => received.extend_from_slice(buffer.get(..read).unwrap_or_default()),
            }
        }
        if let (Ok(text), Ok(mut guard)) = (std::str::from_utf8(&received), heard.lock()) {
            guard.push(text.to_owned());
        }

        let _ = socket.write_all(reply.as_bytes()).await;
        let _ = socket.flush().await;
        let _ = socket.shutdown().await;
    }
}

/// A response that sends the caller somewhere else.
fn moved(status: u16, to: &str) -> String {
    format!("HTTP/1.1 {status} Moved\r\nLocation: {to}/taken\r\nContent-Length: 0\r\n\r\n")
}

/// A response that answers rather than deflects.
fn answered() -> String {
    "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nhi".to_owned()
}

/// A request as every \*arr call in this product makes one: the credential in a
/// header, and — as an indexer authenticates — a second one in the query.
fn asking(base: &str, key: &str) -> Request {
    Request {
        method: Method::Get,
        url: format!("{base}/api/v3/system/status?apikey={key}"),
        headers: vec![(API_KEY_HEADER.to_owned(), key.to_owned())],
        body: None,
    }
}

/// A service cannot send this machine to a host nobody wrote down.
///
/// The whole of the closed-host promise rests on this. Every service in the stack
/// is somebody else's container image reached over loopback, and any one of them —
/// compromised, or merely proxied through something that is — can answer a request
/// with an address of its choosing. If that address is followed, the enumeration
/// that reads source text passes while the request leaves for a host it has never
/// heard of.
///
/// Held over every redirect status rather than the one a test author thought of,
/// because they are not one behaviour: `303` rewrites the method, `307` and `308`
/// preserve it, and a policy that handled some and not others would read as though
/// it had closed this.
#[tokio::test]
async fn a_service_cannot_send_this_machine_to_a_host_it_was_not_addressed_to() {
    for status in [301u16, 302, 303, 307, 308] {
        let elsewhere = serve(answered()).await;
        let service = serve(moved(status, &elsewhere.base)).await;

        let _ = Web::new().send(&asking(&service.base, &credential())).await;

        let arrived = elsewhere.reached();
        let asked = service.reached();
        elsewhere.stop().await;
        service.stop().await;

        assert_eq!(asked, 1, "{status}: the service itself was asked once");
        assert_eq!(
            arrived, 0,
            "{status}: the service named another host and this machine went there — the list \
             of hosts anything reaches is closed only if a request arrives where it was \
             addressed"
        );
    }
}

/// And the half that says what the hop would have cost.
///
/// A connection count proves the request did not arrive; this proves what it would
/// have carried if it had. The credential a \*arr authenticates with travels in
/// `X-Api-Key`, which is not one of the three headers a redirect strips, and an
/// indexer's travels in the query, which nothing strips at all. Both are asserted
/// against the bytes the second host actually received, so this stays true however
/// the client comes to be built.
#[tokio::test]
async fn no_credential_a_request_carries_reaches_a_host_that_asked_for_it() {
    let key = credential();
    let elsewhere = serve(answered()).await;
    let service = serve(moved(302, &elsewhere.base)).await;

    let _ = Web::new().send(&asking(&service.base, &key)).await;

    let heard = elsewhere.heard();
    elsewhere.stop().await;
    service.stop().await;

    assert!(
        !heard.contains(&key),
        "a host this product never named was handed the service's credential: {heard:?}"
    );
    assert!(
        !heard.to_lowercase().contains("x-api-key"),
        "and was sent the credential header at all: {heard:?}"
    );
    assert!(
        heard.is_empty(),
        "nothing whatever should have reached it: {heard:?}"
    );
}

/// A hop a service asks for comes back as the answer it is.
///
/// The other half of refusing one: it must be *visible*. `outbound.log` records the
/// request as submitted, so a hop taken quietly would leave that file stating the
/// opposite of what happened — the one file an operator is invited to read to check
/// the enumeration. Returned as a status, the deflection is in the record and in
/// whatever the caller decides it means.
#[tokio::test]
async fn a_hop_a_service_asks_for_is_the_answer_rather_than_a_thing_taken_quietly() {
    let elsewhere = serve(answered()).await;
    let service = serve(moved(302, &elsewhere.base)).await;

    let answer = Web::new().send(&asking(&service.base, &credential())).await;

    elsewhere.stop().await;
    service.stop().await;

    assert_eq!(
        answer.ok().map(|response| response.status),
        Some(302),
        "the redirect is what the service said, and saying so is what puts it in the record"
    );
}
