use std::time::Duration;

use axum::http::{header, StatusCode};
use axum::routing::get;
use axum::Router;

use super::fake::{tls, tls12};
use super::{client, presents, sha256};

/// Long enough for a loopback server, short enough that a test never hangs.
const WITHIN: Duration = Duration::from_secs(10);

fn answering() -> Router {
    Router::new().route("/", get(|| async { "pinned" })).route(
        "/elsewhere",
        get(|| async {
            (
                StatusCode::FOUND,
                [(header::LOCATION, "https://example.invalid/")],
            )
        }),
    )
}

#[test]
fn a_pin_is_the_sha256_of_the_whole_certificate_in_lower_case_hex() {
    assert_eq!(
        sha256(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[tokio::test]
async fn a_service_presenting_a_pinned_certificate_is_reached_and_any_other_is_refused() {
    let served = tls(answering());
    let Some(served) = served else {
        unreachable!("the fake serves");
    };
    let pinned = client(std::slice::from_ref(&served.pin), Some(WITHIN));
    let Some(pinned) = pinned else {
        unreachable!("a pinned client is built");
    };
    let answer = pinned.get(&served.base).send().await;
    let body = match answer {
        Ok(answer) => answer.text().await.ok(),
        Err(_) => None,
    };
    assert_eq!(body.as_deref(), Some("pinned"));

    let other = client(&["0".repeat(64)], None);
    assert!(other.is_some());
    if let Some(other) = other {
        assert!(other.get(&served.base).send().await.is_err());
    }
    assert!(client(&[], Some(WITHIN)).is_none());
}

#[tokio::test]
async fn a_redirect_is_answered_rather_than_followed_and_plain_http_is_refused() {
    let served = tls12(answering());
    let Some(served) = served else {
        unreachable!("the fake serves");
    };
    let pinned = client(std::slice::from_ref(&served.pin), Some(WITHIN));
    let Some(pinned) = pinned else {
        unreachable!("a pinned client is built");
    };
    let status = pinned
        .get(format!("{}/elsewhere", served.base))
        .send()
        .await
        .map(|answer| answer.status());
    assert_eq!(status.ok(), Some(StatusCode::FOUND));
    let plain = served.base.replacen("https://", "http://", 1);
    assert!(pinned.get(plain).send().await.is_err());
}

#[tokio::test]
async fn a_handshake_presents_a_pin_only_where_the_certificate_is_that_one() {
    let served = tls(answering());
    let Some(served) = served else {
        unreachable!("the fake serves");
    };
    assert!(presents(&served.base, &served.pin, WITHIN).await);
    assert!(!presents(&served.base, &"0".repeat(64), WITHIN).await);
    assert!(!presents(&served.base, "", WITHIN).await);
}
