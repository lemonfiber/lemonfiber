//! What a request is answered with, and what it is refused for.
//!
//! Driven from outside the crate, because what a caller can reach is the thing
//! worth holding still.

use std::net::SocketAddr;

use axum::body::{to_bytes, Body};
use axum::http::{header, HeaderMap, HeaderValue, Response, StatusCode};
use lemonfiber_api::guard::{Token, TOKEN_HEADER};
use lemonfiber_api::read::enveloped;
use lemonfiber_api::serve::{admitted, answered, refused, token_header, Refusal};
use lemonfiber_fixtures::ports::Chance;

/// Bytes the test chose, so a token is the same one twice.
///
/// Cycled to whatever width is asked for rather than fixed at some other one: a
/// source that answers short mints no token at all, which would quietly turn
/// every test here into a test of that instead.
fn given() -> Chance {
    Chance::cycling()
}

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

/// Whether the secret a request carried is this run's token.
///
/// The other secret a request may carry is a session, which is opened by proving a
/// password and is [`lemonfiber_api::admission`]'s business rather than this file's.
/// What is settled here is the shape of the decision the guard makes over whichever
/// of the two answered.
fn this_runs(headers: &HeaderMap, token: &Token) -> bool {
    token.carried_by(
        headers
            .get(TOKEN_HEADER)
            .and_then(|value| value.to_str().ok()),
    )
}

/// The verdict on a request saying exactly what the test states, and nothing more.
fn verdict(pairs: &[(&str, &str)]) -> Result<(), Refusal> {
    Token::mint(&given()).map_or(Err(Refusal::Unknown), |token| {
        let headers = saying(pairs);
        admitted(this_runs(&headers, &token), &headers, bound())
    })
}

/// The verdict on a request that carries this run's token, plus what the test states.
///
/// The token is read back from the run that minted it rather than written out
/// here, so a test states what it is about — the address — and no line of this
/// file has to be kept in step with how a secret is written.
fn carrying_the_token(pairs: &[(&str, &str)]) -> Result<(), Refusal> {
    let Some(token) = Token::mint(&given()) else {
        return Err(Refusal::Unknown);
    };
    let mut every = vec![(TOKEN_HEADER, token.as_str())];
    every.extend_from_slice(pairs);
    let headers = saying(&every);
    admitted(this_runs(&headers, &token), &headers, bound())
}

#[test]
fn a_request_carrying_the_token_from_here_is_answered() {
    assert_eq!(
        carrying_the_token(&[
            ("host", "127.0.0.1:8471"),
            ("origin", "http://localhost:8471"),
        ]),
        Ok(())
    );
}

#[test]
fn a_browser_that_states_no_origin_is_still_answered() {
    assert_eq!(carrying_the_token(&[("host", "localhost:8471")]), Ok(()));
}

#[test]
fn a_request_carrying_no_token_is_not() {
    assert_eq!(
        verdict(&[("host", "localhost:8471")]),
        Err(Refusal::Unknown)
    );
}

#[test]
fn a_request_carrying_another_token_of_the_same_width_is_not() {
    let Some(token) = Token::mint(&given()) else {
        unreachable!("the source above always answers");
    };
    // As long as the real one, so what is refused is the value and not the shape.
    let other = token.as_str().replace('a', "b");
    assert_eq!(other.len(), token.as_str().len());
    assert_eq!(
        verdict(&[(TOKEN_HEADER, &other), ("host", "localhost:8471")]),
        Err(Refusal::Unknown)
    );
}

#[test]
fn a_request_naming_another_address_is_not() {
    assert_eq!(
        carrying_the_token(&[("host", "example.com:8471")]),
        Err(Refusal::Elsewhere)
    );
}

#[test]
fn a_request_sent_from_another_page_is_not() {
    assert_eq!(
        carrying_the_token(&[
            ("host", "localhost:8471"),
            ("origin", "http://elsewhere.example:8471"),
        ]),
        Err(Refusal::Elsewhere)
    );
}

#[test]
fn a_request_naming_no_address_at_all_is_not() {
    assert_eq!(carrying_the_token(&[]), Err(Refusal::Elsewhere));
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

/// Each response this surface builds from a body it already holds.
///
/// The stream is built through the same call and is driven where a listener can
/// be made; these are the three a test can put together in one line.
fn each_response() -> [(&'static str, Response<Body>); 3] {
    [
        ("an answer", answered(r#"{"api_version":1}"#.to_owned())),
        ("a refusal", refused(Refusal::Unknown)),
        (
            "an answer that would not render",
            enveloped(StatusCode::OK, None),
        ),
    ]
}

#[test]
fn nothing_this_surface_says_may_be_guessed_at() {
    for (which, response) in each_response() {
        assert_eq!(
            response.headers().get(header::X_CONTENT_TYPE_OPTIONS),
            Some(&HeaderValue::from_static("nosniff")),
            "{which} let a browser decide for itself what it had been given"
        );
    }
}

#[test]
fn nothing_this_surface_says_may_be_kept() {
    for (which, response) in each_response() {
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL),
            Some(&HeaderValue::from_static("no-store")),
            "{which} was allowed to be stored"
        );
    }
}

#[test]
fn what_this_surface_says_in_its_own_words_is_labelled_as_prose() {
    // Not as an envelope: a caller that parses what it was told it was given
    // would otherwise be handed a sentence to read as JSON.
    for response in [refused(Refusal::Elsewhere), enveloped(StatusCode::OK, None)] {
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE),
            Some(&HeaderValue::from_static("text/plain; charset=utf-8"))
        );
    }
}
