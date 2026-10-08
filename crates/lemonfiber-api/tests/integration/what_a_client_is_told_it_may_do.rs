//! What a client is told this stack can do, as the credential it holds may do it.
//!
//! Driven through the route rather than by calling the function behind it, because
//! what a phone reads is the envelope the route answers with: the kind it names, and
//! each capability under the path its request is served at.

use std::sync::Arc;

use axum::body::to_bytes;
use axum::http::StatusCode;
use axum::Extension;
use lemonfiber_api::admission::Caller;
use lemonfiber_api::events::live::Live;
use lemonfiber_api::guard::Token;
use lemonfiber_api::jobs::Jobs;
use lemonfiber_api::router::Serving;
use lemonfiber_fixtures::ports::{Chance, Stopped};

/// The capabilities route as a run builds it, asked by `caller`.
fn routed(caller: Caller) -> axum::Router {
    let Some(token) = Token::mint(&Chance::cycling()) else {
        unreachable!("cycling letters always supply bytes");
    };
    lemonfiber_api::capabilities::routes()
        .with_state(Serving {
            ctx: Arc::new(lemonfiber_testing::a_context().build()),
            token: Arc::new(token),
            bound: lemonfiber_api::guard::Binding::here(8471),
            admitting: Arc::new(lemonfiber_api::admission::Admitting::default()),
            jobs: Jobs::default(),
            live: Arc::new(Live::opening(Stopped::at(0).as_ref())),
            kept: Arc::default(),
            answered: Arc::default(),
        })
        // The subject the guard puts on every request it admits, mounted here because
        // a test builds this route without the layer that carries it.
        .layer(Extension(caller))
}

/// What the route answered `caller` with: its status and its body.
async fn told(caller: Caller) -> (StatusCode, serde_json::Value) {
    let request = axum::http::Request::builder()
        .method("GET")
        .uri(lemonfiber_api::capabilities::CAPABILITIES)
        .body(axum::body::Body::empty());
    let Ok(request) = request else {
        unreachable!("a request built from values that are already a URI cannot fail");
    };
    let Ok(response) = tower::ServiceExt::oneshot(routed(caller), request).await;
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap_or_default();
    (status, serde_json::from_slice(&body).unwrap_or_default())
}

/// What `said` says of the capability served at `path`, or nothing where it is absent.
fn standing<'a>(said: &'a serde_json::Value, path: &str) -> Option<&'a str> {
    said.get("data")?.get("capabilities")?.get(path)?.as_str()
}

#[tokio::test]
async fn the_operator_is_told_every_request_is_available() {
    let (status, said) = told(Caller::Operator).await;
    assert_eq!(status, StatusCode::OK, "{said}");
    assert_eq!(
        said.get("kind").and_then(serde_json::Value::as_str),
        Some("capabilities"),
        "{said}"
    );
    assert_eq!(
        standing(&said, "/api/actions/pull"),
        Some("available"),
        "{said}"
    );
    assert_eq!(standing(&said, "/api/plugins"), Some("available"), "{said}");
}

#[tokio::test]
async fn a_member_is_told_what_is_theirs_and_what_is_not() {
    let (status, said) = told(Caller::Member("someone".to_owned())).await;
    assert_eq!(status, StatusCode::OK, "{said}");
    assert_eq!(standing(&said, "/api/held"), Some("available"), "{said}");
    assert_eq!(
        standing(&said, "/api/actions/pull"),
        Some("unpermitted"),
        "{said}"
    );
    assert_eq!(
        standing(&said, "/api/actions/no-such-action"),
        None,
        "a request this stack does not have is absent: {said}"
    );
}
