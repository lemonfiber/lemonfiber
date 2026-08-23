//! What a request is answered with, and what it is refused for.
//!
//! Driven from outside the crate, because what a caller can reach is the thing
//! worth holding still.

use std::net::SocketAddr;

use axum::body::to_bytes;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use lemonfiber_api::guard::{Token, TOKEN_HEADER};
use lemonfiber_api::serve::{admitted, answered, refused, token_header, Refusal};
use lemonfiber_core::ports::random::Random;

/// Hands back bytes the test chose, so a token is the same one twice.
struct Given;

impl Random for Given {
    fn bytes(&self, _: usize) -> Option<Vec<u8>> {
        Some(vec![0x00, 0x0f, 0xa5, 0xff])
    }
}

/// The token every test here admits, and its written form.
const WRITTEN: &str = "000fa5ff";

fn bound() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 8471))
}

/// A request saying what a browser on this machine would say.
fn saying(pairs: &[(&str, &str)]) -> HeaderMap {
    let mut headers = HeaderMap::new();
    for (name, value) in pairs {
        if let Ok(value) = HeaderValue::from_str(value) {
            headers.insert(
                axum::http::HeaderName::from_bytes(name.to_lowercase().as_bytes())
                    .unwrap_or(header::HOST),
                value,
            );
        }
    }
    headers
}

fn verdict(pairs: &[(&str, &str)]) -> Result<(), Refusal> {
    Token::mint(&Given).map_or(Err(Refusal::Unknown), |token| {
        admitted(&saying(pairs), &token, bound())
    })
}

#[test]
fn a_request_carrying_the_token_from_here_is_answered() {
    assert_eq!(
        verdict(&[
            (TOKEN_HEADER, WRITTEN),
            ("host", "127.0.0.1:8471"),
            ("origin", "http://localhost:8471"),
        ]),
        Ok(())
    );
}

#[test]
fn a_browser_that_states_no_origin_is_still_answered() {
    assert_eq!(
        verdict(&[(TOKEN_HEADER, WRITTEN), ("host", "localhost:8471")]),
        Ok(())
    );
}

#[test]
fn a_request_carrying_no_token_is_not() {
    assert_eq!(
        verdict(&[("host", "localhost:8471")]),
        Err(Refusal::Unknown)
    );
}

#[test]
fn a_request_carrying_another_token_is_not() {
    assert_eq!(
        verdict(&[(TOKEN_HEADER, "000fa5fe"), ("host", "localhost:8471")]),
        Err(Refusal::Unknown)
    );
}

#[test]
fn a_request_naming_another_address_is_not() {
    assert_eq!(
        verdict(&[(TOKEN_HEADER, WRITTEN), ("host", "example.com:8471")]),
        Err(Refusal::Elsewhere)
    );
}

#[test]
fn a_request_sent_from_another_page_is_not() {
    assert_eq!(
        verdict(&[
            (TOKEN_HEADER, WRITTEN),
            ("host", "localhost:8471"),
            ("origin", "http://elsewhere.example:8471"),
        ]),
        Err(Refusal::Elsewhere)
    );
}

#[test]
fn a_request_naming_no_address_at_all_is_not() {
    assert_eq!(verdict(&[(TOKEN_HEADER, WRITTEN)]), Err(Refusal::Elsewhere));
}

#[tokio::test]
async fn an_answer_carries_the_envelope_it_was_given() {
    let rendered = r#"{"api_version":1,"kind":"status","data":{}}"#;
    let response = answered(rendered.to_owned());

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CONTENT_TYPE),
        Some(&HeaderValue::from_static("application/json"))
    );
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL),
        Some(&HeaderValue::from_static("no-store"))
    );

    let body = to_bytes(response.into_body(), usize::MAX).await;
    assert_eq!(body.ok().as_deref(), Some(rendered.as_bytes()));
}

#[tokio::test]
async fn a_refusal_says_which_of_the_two_it_was() {
    for refusal in [Refusal::Unknown, Refusal::Elsewhere] {
        let response = refused(refusal);
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let body = to_bytes(response.into_body(), usize::MAX).await;
        assert_eq!(body.ok().as_deref(), Some(refusal.said().as_bytes()));
    }
}

#[test]
fn the_two_refusals_do_not_say_the_same_thing() {
    assert_ne!(Refusal::Unknown.said(), Refusal::Elsewhere.said());
}

#[test]
fn the_header_a_caller_must_use_is_the_one_the_contract_names() {
    assert_eq!(token_header(), "X-Lemonfiber-Token");
}
