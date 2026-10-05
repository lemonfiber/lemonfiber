use std::time::Duration;

use axum::extract::ConnectInfo;
use axum::routing::{get, post};
use axum::Router;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;

use super::{answering, Accepting, Limits, Shares};

/// Tight bounds, so each case below takes a fraction of a second.
const TIGHT: Limits = Limits {
    headers_within: Duration::from_millis(300),
    body_within: Duration::from_millis(300),
    shares: Shares {
        at_once: 2,
        each_peer: 2,
        kept_for_here: 0,
    },
    let_go: Duration::from_millis(300),
};

/// A surface with one route that answers, one that never finishes, one that reads
/// what it is sent and one that says where a request came from.
fn surface() -> Router {
    Router::new()
        .route("/", get(|| async { "answered" }))
        .route(
            "/forever",
            get(|| async { std::future::pending::<&'static str>().await }),
        )
        .route(
            "/read",
            post(
                |body: Result<String, axum::extract::rejection::StringRejection>| async move {
                    match body {
                        Ok(read) => format!("read {} bytes", read.len()),
                        Err(why) => format!("not read: {why}"),
                    }
                },
            ),
        )
        .route(
            "/from",
            get(
                |ConnectInfo(from): ConnectInfo<std::net::SocketAddr>| async move {
                    format!("from {}", from.ip())
                },
            ),
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
    let running = tokio::spawn(answering(
        Box::new(listener),
        surface(),
        stopped,
        limits,
        None,
    ));
    Some((at, stop, running))
}

/// A listener whose first accept fails, the way one out of descriptors does.
struct FailingOnce {
    /// Whether the failure has been given yet.
    failed: std::sync::atomic::AtomicBool,
    /// Where every later connection comes from.
    listener: TcpListener,
}

#[async_trait::async_trait]
impl Accepting for FailingOnce {
    async fn accept(&self) -> std::io::Result<(TcpStream, std::net::SocketAddr)> {
        if !self.failed.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return Err(std::io::Error::other("out of descriptors"));
        }
        self.listener.accept().await
    }
}

/// What a plain request to `at` is answered with.
async fn answer_at(at: std::net::SocketAddr) -> String {
    sent(
        at,
        b"GET / HTTP/1.1\r\nhost: here\r\nconnection: close\r\n\r\n",
    )
    .await
}

/// What `at` answers these bytes with.
async fn sent(at: std::net::SocketAddr, request: &[u8]) -> String {
    let mut answer = String::new();
    if let Ok(mut stream) = TcpStream::connect(at).await {
        if stream.write_all(request).await.is_ok() {
            let _ = stream.read_to_string(&mut answer).await;
        }
    }
    answer
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
    let answer = answer_at(at).await;
    assert!(answer.contains("answered"), "{answer}");
}

/// A failed accept is waited out rather than ending the socket or spinning on it.
#[tokio::test]
async fn a_failed_accept_leaves_the_socket_answering() {
    let Ok(listener) = TcpListener::bind("127.0.0.1:0").await else {
        unreachable!("a loopback socket could not be bound");
    };
    let Ok(at) = listener.local_addr() else {
        unreachable!("a bound socket has an address");
    };
    let (_stop, stopped) = watch::channel(false);
    let failing = FailingOnce {
        failed: std::sync::atomic::AtomicBool::new(false),
        listener,
    };
    let _running = tokio::spawn(answering(
        Box::new(failing),
        surface(),
        stopped,
        TIGHT,
        None,
    ));

    let answer = answer_at(at).await;
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
    for _ in 0..limits.shares.at_once {
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

/// A body promised and sent a byte at a time is cut off rather than waited for.
#[tokio::test]
async fn a_body_that_never_finishes_arriving_is_cut_off() {
    let Some((at, _stop, _running)) = serving(TIGHT).await else {
        unreachable!("a loopback socket could not be bound");
    };
    let Ok(mut stream) = TcpStream::connect(at).await else {
        unreachable!("the socket did not accept");
    };
    assert!(stream
        .write_all(b"POST /read HTTP/1.1\r\nhost: here\r\ncontent-length: 2000000\r\n\r\nx")
        .await
        .is_ok());
    let mut answer = String::new();
    let read = tokio::time::timeout(Duration::from_secs(3), stream.read_to_string(&mut answer));
    assert!(
        read.await.is_ok(),
        "a body that never arrived held its socket"
    );
    assert!(answer.contains("did not arrive in time"), "{answer}");
}

/// A body that does arrive in time is read whole.
#[tokio::test]
async fn a_body_that_arrives_in_time_is_read() {
    let Some((at, _stop, _running)) = serving(TIGHT).await else {
        unreachable!("a loopback socket could not be bound");
    };
    let answer = sent(
        at,
        b"POST /read HTTP/1.1\r\nhost: here\r\ncontent-length: 5\r\nconnection: close\r\n\r\nhello",
    )
    .await;
    assert!(answer.contains("read 5 bytes"), "{answer}");
}

/// Every request carries where it came from, for the routes that count by address.
#[tokio::test]
async fn a_request_says_where_it_came_from() {
    let Some((at, _stop, _running)) = serving(TIGHT).await else {
        unreachable!("a loopback socket could not be bound");
    };
    let answer = sent(
        at,
        b"GET /from HTTP/1.1\r\nhost: here\r\nconnection: close\r\n\r\n",
    )
    .await;
    assert!(answer.contains("from 127.0.0.1"), "{answer}");
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

/// Encrypted answering, as a paired phone meets it.
mod encrypted {
    use std::sync::Arc;

    use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
    use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
    use rustls::{DigitallySignedStruct, SignatureScheme};
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::watch;

    use super::{answer_at, surface, TIGHT};
    use crate::ui::answering::answering;
    use crate::ui::encrypted::{encrypting, Encrypting};

    /// A client that trusts one certificate by its digest and nothing else, which is
    /// what a paired phone is.
    #[derive(Debug)]
    struct Pinned(String);

    impl Pinned {
        /// The algorithms a signature may be checked with.
        fn algorithms() -> rustls::crypto::WebPkiSupportedAlgorithms {
            rustls::crypto::ring::default_provider().signature_verification_algorithms
        }
    }

    impl ServerCertVerifier for Pinned {
        fn verify_server_cert(
            &self,
            presented: &CertificateDer<'_>,
            _intermediates: &[CertificateDer<'_>],
            _name: &ServerName<'_>,
            _stapled: &[u8],
            _now: UnixTime,
        ) -> Result<ServerCertVerified, rustls::Error> {
            if lemonfiber_core::companion::certificate::fingerprint(presented) == self.0 {
                Ok(ServerCertVerified::assertion())
            } else {
                Err(rustls::Error::General(
                    "not the certificate pinned".to_owned(),
                ))
            }
        }

        fn verify_tls12_signature(
            &self,
            message: &[u8],
            presented: &CertificateDer<'_>,
            signed: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, rustls::Error> {
            rustls::crypto::verify_tls12_signature(message, presented, signed, &Self::algorithms())
        }

        fn verify_tls13_signature(
            &self,
            message: &[u8],
            presented: &CertificateDer<'_>,
            signed: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, rustls::Error> {
            rustls::crypto::verify_tls13_signature(message, presented, signed, &Self::algorithms())
        }

        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
            Self::algorithms().supported_schemes()
        }
    }

    /// What this run presents, from a certificate kept in a directory of its own.
    fn presenting(named: &str) -> Option<Encrypting> {
        let at = lemonfiber_fixtures::scratch::Scratch::new(named).kept();
        let ctx = lemonfiber_testing::context::a_context()
            .settings(lemonfiber_core::config::Settings {
                companion: Some(at),
                ..lemonfiber_core::config::Settings::default()
            })
            .build();
        encrypting(&ctx, true, Some(1)).ok().flatten()
    }

    /// An encrypted socket being answered, and the switch that stops it.
    async fn serving_encrypted(
        on: &Encrypting,
    ) -> Option<(std::net::SocketAddr, watch::Sender<bool>)> {
        let listener = TcpListener::bind("127.0.0.1:0").await.ok()?;
        let at = listener.local_addr().ok()?;
        let (stop, stopped) = watch::channel(false);
        tokio::spawn(answering(
            Box::new(listener),
            surface(),
            stopped,
            TIGHT,
            Some(on.acceptor.clone()),
        ));
        Some((at, stop))
    }

    /// What a client pinning `fingerprint` is answered with, or nothing where it would
    /// not connect.
    async fn pinned_answer(at: std::net::SocketAddr, fingerprint: &str) -> Option<String> {
        let config = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .ok()?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(Pinned(fingerprint.to_owned())))
        .with_no_client_auth();
        let socket = TcpStream::connect(at).await.ok()?;
        let name = ServerName::try_from("localhost").ok()?;
        let mut stream = tokio_rustls::TlsConnector::from(Arc::new(config))
            .connect(name, socket)
            .await
            .ok()?;
        stream
            .write_all(b"GET / HTTP/1.1\r\nhost: here\r\nconnection: close\r\n\r\n")
            .await
            .ok()?;
        let mut answer = String::new();
        let _ = stream.read_to_string(&mut answer).await;
        Some(answer)
    }

    /// A client pinning the certificate this run presents is answered.
    #[tokio::test]
    async fn a_client_pinning_the_certificate_is_answered() {
        let on = presenting("answering-encrypted");
        let served = match &on {
            Some(on) => serving_encrypted(on).await,
            None => None,
        };
        let answer = match (&on, &served) {
            (Some(on), Some((at, _))) => pinned_answer(*at, &on.fingerprint).await,
            _ => None,
        };
        assert!(
            answer.is_some_and(|said| said.contains("answered")),
            "a phone that pinned this certificate is answered"
        );
        let refused = match &served {
            Some((at, _)) => pinned_answer(*at, &"00".repeat(32)).await,
            None => Some(String::new()),
        };
        assert_eq!(refused, None, "and one that pinned another refuses it");
    }

    /// Plain text sent to an encrypted socket is not answered, and the socket goes on
    /// answering the next connection.
    #[tokio::test]
    async fn plain_text_to_an_encrypted_socket_is_not_answered() {
        let on = presenting("answering-encrypted-plain");
        let served = match &on {
            Some(on) => serving_encrypted(on).await,
            None => None,
        };
        let plain = match &served {
            Some((at, _)) => answer_at(*at).await,
            None => "no socket".to_owned(),
        };
        assert!(!plain.contains("answered"), "{plain}");
        let after = match (&on, &served) {
            (Some(on), Some((at, _))) => pinned_answer(*at, &on.fingerprint).await,
            _ => None,
        };
        assert!(after.is_some_and(|said| said.contains("answered")));
    }
}
