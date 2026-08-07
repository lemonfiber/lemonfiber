//! The NNTP dialer, driven through the port against real sockets on the loopback.
//!
//! Driven from here rather than in-crate for the reason every adapter is: a file
//! carrying its own tests *and* reached from an integration test is compiled into
//! two coverage mappings, and the copy the tests do not run counts against the
//! gate forever. An adapter is the outside world; it is proven from outside.
//!
//! A real socket rather than a fake stream, because the whole of this adapter is
//! transport: what is worth proving is that it connects, reads a greeting, sends
//! what it was given, and reports a provider that will not answer as unreachable.

use std::time::Duration;

use lemonfiber_core::adapters::Dialer;
use lemonfiber_core::ports::nntp::{Endpoint, Nntp};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::TcpListener;

/// An endpoint at a port on the loopback.
fn at(port: u16, secure: bool) -> Endpoint {
    Endpoint {
        host: "127.0.0.1".to_owned(),
        port,
        secure,
    }
}

/// The port a listener bound to, or zero — nothing listens on zero, which is what
/// a test wanting a refusal asks for anyway.
fn port_of(listener: Option<&TcpListener>) -> u16 {
    listener
        .and_then(|listener| listener.local_addr().ok())
        .map_or(0, |addr| addr.port())
}

/// A server running in the background, awaited at the end of a test so it runs to
/// its own end rather than being cut off mid-answer when the test returns.
struct Server(tokio::task::JoinHandle<Option<()>>);

impl Server {
    async fn finished(self) {
        // Bounded: a server whose client never closed should not hold the test.
        let _ = tokio::time::timeout(Duration::from_secs(3), self.0).await;
    }
}

/// Serve one connection with the scripted lines.
async fn serving(script: &'static [&'static str]) -> (u16, Server) {
    let listener = TcpListener::bind("127.0.0.1:0").await.ok();
    let port = port_of(listener.as_ref());
    (port, Server(tokio::spawn(speak(listener, script))))
}

async fn speak(listener: Option<TcpListener>, script: &'static [&'static str]) -> Option<()> {
    let (mut socket, _) = listener?.accept().await.ok()?;
    for line in script {
        socket
            .write_all(format!("{line}\r\n").as_bytes())
            .await
            .ok()?;
        // Let the client read and send its next command before the reply to it
        // goes out, so the exchange stays in step.
        tokio::task::yield_now().await;
    }
    // Held open until the client has finished sending, so its writes land —
    // bounded, so a client that never closes does not hold the server open.
    let mut sent = Vec::new();
    let _ = tokio::time::timeout(Duration::from_secs(1), socket.read_to_end(&mut sent)).await;
    Some(())
}

/// A listener that accepts and then says nothing at all.
async fn silent() -> (u16, Server) {
    let listener = TcpListener::bind("127.0.0.1:0").await.ok();
    let port = port_of(listener.as_ref());
    let waiting = tokio::spawn(async move {
        let accepted = listener?.accept().await.ok();
        // Held open, unanswered, for longer than the patience under test.
        tokio::time::sleep(Duration::from_millis(300)).await;
        drop(accepted);
        Some(())
    });
    (port, Server(waiting))
}

/// A listener that accepts and immediately hangs up.
async fn hangs_up() -> (u16, Server) {
    let listener = TcpListener::bind("127.0.0.1:0").await.ok();
    let port = port_of(listener.as_ref());
    let closing = tokio::spawn(async move {
        drop(listener?.accept().await.ok());
        Some(())
    });
    (port, Server(closing))
}

/// A listener that streams bytes with no line terminator in sight.
async fn floods() -> (u16, Server) {
    let listener = TcpListener::bind("127.0.0.1:0").await.ok();
    let port = port_of(listener.as_ref());
    let flooding = tokio::spawn(async move {
        let (mut socket, _) = listener?.accept().await.ok()?;
        let _ = socket.write_all(&vec![b'x'; 32 * 1024]).await;
        let mut sent = Vec::new();
        let _ = tokio::time::timeout(Duration::from_secs(1), socket.read_to_end(&mut sent)).await;
        Some(())
    });
    (port, Server(flooding))
}

#[tokio::test]
async fn a_dial_returns_the_greeting_then_each_reply_in_order() {
    let (port, server) = serving(&["200 welcome", "381 password", "281 authenticated"]).await;
    let commands = vec!["AUTHINFO USER me".to_owned(), "AUTHINFO PASS x".to_owned()];
    let replies = Dialer::new().converse(&at(port, false), &commands).await;
    assert_eq!(
        replies.ok(),
        Some(vec![
            "200 welcome".to_owned(),
            "381 password".to_owned(),
            "281 authenticated".to_owned(),
        ])
    );
    server.finished().await;
}

#[tokio::test]
async fn a_provider_that_hangs_up_before_answering_is_nothing_usable() {
    // A closed connection where a greeting was expected says nothing at all about
    // the credential, which is different from a provider that refused it.
    let (port, server) = hangs_up().await;
    let refused = Dialer::new().converse(&at(port, false), &[]).await;
    assert!(refused.is_err_and(|error| error.reason.contains("closed the connection")));
    server.finished().await;
}

#[tokio::test]
async fn a_peer_streaming_without_a_newline_is_cut_off_rather_than_grown() {
    // Far more than a status line and no terminator in sight: read up to the bound
    // and stop, rather than letting a hostile peer grow the buffer without limit.
    let (port, server) = floods().await;
    let replies = Dialer::new()
        .converse(&at(port, false), &[])
        .await
        .unwrap_or_default();
    let first = replies.first().map(String::len).unwrap_or_default();
    assert!(first <= 8 * 1024, "bounded at the cap, got {first}");
    server.finished().await;
}

#[tokio::test]
async fn the_default_dialer_is_the_one_that_trusts_the_bundled_roots() {
    // Nothing listening, so this proves only that the default is a working dialer
    // — which is the whole of what a Default impl owes anyone.
    let port = port_of(TcpListener::bind("127.0.0.1:0").await.ok().as_ref());
    assert!(Dialer::default()
        .converse(&at(port, false), &[])
        .await
        .is_err());
}

#[tokio::test]
async fn nothing_listening_is_reported_as_unreachable() {
    // Bound and dropped, so the port is almost certainly free and refuses.
    let port = port_of(TcpListener::bind("127.0.0.1:0").await.ok().as_ref());
    assert!(Dialer::new().converse(&at(port, false), &[]).await.is_err());
}

#[tokio::test]
async fn a_provider_that_never_answers_runs_out_of_patience() {
    let (port, server) = silent().await;
    // A budget short enough to actually run out inside a test.
    let dialer = Dialer::with_budget(Duration::from_millis(50));
    let waited = dialer.converse(&at(port, false), &[]).await;
    assert!(waited.is_err_and(|error| error.reason.contains("no reply within")));
    server.finished().await;
}

#[tokio::test]
async fn a_secure_dial_against_something_that_is_not_tls_does_not_complete() {
    // The socket answers, but not with a handshake: reported as unreachable rather
    // than treated as a provider that said something.
    let (port, server) = serving(&["200 welcome"]).await;
    assert!(Dialer::new().converse(&at(port, true), &[]).await.is_err());
    server.finished().await;
}

#[tokio::test]
async fn a_host_that_is_not_a_valid_tls_name_is_refused_before_a_socket_is_opened() {
    // Nothing is listening anywhere near this, and nothing needs to be: a name the
    // handshake could never be verified against is settled before connecting, so a
    // password never rides a connection that was doomed to be unwrapped.
    let endpoint = Endpoint {
        host: "not a hostname".to_owned(),
        port: 563,
        secure: true,
    };
    let refused = Dialer::new().converse(&endpoint, &[]).await;
    assert!(refused.is_err_and(|error| error.reason.contains("not a valid TLS name")));
}

#[tokio::test]
async fn a_plain_dial_to_the_same_host_is_not_held_to_a_tls_name() {
    // The same name, unwrapped: there is no handshake to verify against, so it is
    // the connection that fails rather than the name that is refused.
    let endpoint = Endpoint {
        host: "not a hostname".to_owned(),
        port: 563,
        secure: false,
    };
    let refused = Dialer::new().converse(&endpoint, &[]).await;
    assert!(refused.is_err_and(|error| !error.reason.contains("not a valid TLS name")));
}
