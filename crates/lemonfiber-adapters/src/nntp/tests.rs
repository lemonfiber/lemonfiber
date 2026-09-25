use std::time::Duration;

use super::{tls_config, unreachable, Dialer};
use lemonfiber_ports::nntp::{Endpoint, Nntp};

/// The behaviour of this adapter is proven from the core's `tests/integration/nntp.rs`,
/// against real sockets — it is the outside world, and that is where the outside world
/// is reached. What is here is what a library test can settle without one, and it is
/// here at all because this crate is compiled twice: once for its own tests and once
/// for the integration suite to link, and a copy nothing runs counts against the
/// coverage gate whichever copy it is.
#[tokio::test]
async fn a_dialer_reports_a_provider_it_cannot_reach() {
    // Port zero listens nowhere, so this settles the whole path — construct,
    // dial, fail — without needing anything to answer.
    let nowhere = Endpoint {
        host: "127.0.0.1".to_owned(),
        port: 0,
        secure: false,
    };
    assert!(Dialer::new().converse(&nowhere, &[]).await.is_err());
    assert!(Dialer::default().converse(&nowhere, &[]).await.is_err());
    assert!(Dialer::with_budget(Duration::from_millis(50))
        .converse(&nowhere, &[])
        .await
        .is_err());
}

#[tokio::test]
async fn a_secure_dial_settles_an_unusable_name_before_it_connects() {
    let unnameable = Endpoint {
        host: "not a hostname".to_owned(),
        port: 563,
        secure: true,
    };
    let refused = Dialer::new().converse(&unnameable, &[]).await;
    assert!(refused.is_err_and(|error| error.reason.contains("not a valid TLS name")));
}

#[tokio::test]
async fn a_dialer_holds_an_exchange_with_something_that_answers() {
    // A provider that answers, so the exchange and its reply reading are held
    // here as well as from the integration suite: this crate is compiled twice,
    // and each copy has to be run by the tests that share its build.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.ok();
    let port = listener
        .as_ref()
        .and_then(|listener| listener.local_addr().ok())
        .map_or(0, |addr| addr.port());
    let server = tokio::spawn(async move {
        use tokio::io::AsyncWriteExt as _;
        let (mut socket, _) = listener?.accept().await.ok()?;
        socket.write_all(b"200 welcome\r\n").await.ok()?;
        tokio::task::yield_now().await;
        socket.write_all(b"281 authenticated\r\n").await.ok()?;
        let mut sent = Vec::new();
        let _ = tokio::time::timeout(
            Duration::from_secs(1),
            tokio::io::AsyncReadExt::read_to_end(&mut socket, &mut sent),
        )
        .await;
        Some(())
    });

    let answering = Endpoint {
        host: "127.0.0.1".to_owned(),
        port,
        secure: false,
    };
    let replies = Dialer::new()
        .converse(&answering, &["AUTHINFO USER me".to_owned()])
        .await;
    assert_eq!(
        replies.ok(),
        Some(vec![
            "200 welcome".to_owned(),
            "281 authenticated".to_owned()
        ])
    );
    let _ = tokio::time::timeout(Duration::from_secs(3), server).await;
}

#[tokio::test]
async fn a_secure_dial_against_something_that_is_not_tls_does_not_complete() {
    // Reaches the wrapping, which a plain dial never does.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.ok();
    let port = listener
        .as_ref()
        .and_then(|listener| listener.local_addr().ok())
        .map_or(0, |addr| addr.port());
    let server = tokio::spawn(async move {
        use tokio::io::AsyncWriteExt as _;
        let (mut socket, _) = listener?.accept().await.ok()?;
        let _ = socket.write_all(b"200 welcome\r\n").await;
        Some(())
    });
    let secure = Endpoint {
        host: "127.0.0.1".to_owned(),
        port,
        secure: true,
    };
    assert!(Dialer::new().converse(&secure, &[]).await.is_err());
    let _ = tokio::time::timeout(Duration::from_secs(3), server).await;
}

#[test]
fn the_bundled_roots_build_a_configuration() {
    // The trust anchors a static binary carries rather than reading from a
    // system store, so a release with no system TLS still verifies a provider.
    assert!(tls_config().is_ok());
}

#[test]
fn an_unreachable_provider_carries_the_host_and_the_reason() {
    let reported = unreachable("news.example", "refused".to_owned());
    assert_eq!(reported.host, "news.example");
    assert_eq!(reported.reason, "refused");
}
