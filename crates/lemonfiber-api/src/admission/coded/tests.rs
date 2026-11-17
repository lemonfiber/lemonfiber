use std::time::Duration;

use axum::http::StatusCode;
use lemonfiber_core::sign_in_codes::NotKept;

use super::unkept;

#[test]
fn a_code_the_table_would_not_keep_is_refused_as_too_many_or_as_unconfirmed() {
    let waiting = unkept(NotKept::Waiting(Duration::from_secs(90)));
    let undrawn = unkept(NotKept::Undrawn);

    assert_eq!(waiting.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        waiting
            .headers()
            .get(super::super::RETRY_AFTER)
            .and_then(|left| left.to_str().ok()),
        Some("90")
    );
    assert_eq!(undrawn.status(), StatusCode::FORBIDDEN);
}
