use axum::http::StatusCode;
use lemonfiber_error::codes::wish::{NOT_ASKABLE, NOT_CHECKED};
use lemonfiber_error::{Problem, Remedy};

use super::{failed, Reply, RETRY_AFTER};

/// A refusal under `code`.
fn refusal(code: lemonfiber_error::Code) -> Problem {
    Problem::new(code, "Refused.", "Refused.", Remedy::new("Try again."))
}

#[test]
fn a_refusal_whose_code_says_how_long_to_wait_is_answered_with_retry_after_and_others_without() {
    let waits = failed(&refusal(NOT_CHECKED));
    let unknown = failed(&refusal(NOT_ASKABLE));

    assert_eq!(
        (waits.status, waits.retry_after),
        (StatusCode::SERVICE_UNAVAILABLE, Some(5))
    );
    assert_eq!(
        waits
            .response()
            .headers()
            .get(RETRY_AFTER)
            .and_then(|seconds| seconds.to_str().ok()),
        Some("5")
    );
    assert_eq!(unknown.retry_after, None);
    assert!(unknown.response().headers().get(RETRY_AFTER).is_none());
    assert_eq!(Reply::of(StatusCode::OK, None).retry_after, None);
}
