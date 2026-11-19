use std::time::Duration;

use crate::error::codes::admit::{TOO_MANY_CODES, UNCONFIRMED};

use super::unkept;
use crate::sign_in_codes::NotKept;

#[test]
fn a_code_the_table_would_not_keep_is_refused_as_too_many_or_as_unconfirmed() {
    let waiting = unkept(NotKept::Waiting(Duration::from_secs(90)));
    let undrawn = unkept(NotKept::Undrawn);

    assert_eq!(waiting.code, TOO_MANY_CODES);
    assert_eq!(waiting.status(), 429);
    assert_eq!(waiting.retry_after, Some(90));
    assert_eq!(undrawn.code, UNCONFIRMED);
    assert_eq!(undrawn.status(), 403);
    assert_eq!(undrawn.retry_after, None);
}

#[test]
fn a_code_asked_for_is_debugged_without_the_session_or_the_handle() {
    let coded = super::Coded {
        session: Some("s3ss10n".to_owned()),
        from: None,
        waiting: Some("h4ndl3".to_owned()),
    };
    let said = format!("{coded:?}");
    assert!(said.contains("finishing: true"), "{said}");
    assert!(
        !said.contains("s3ss10n") && !said.contains("h4ndl3"),
        "{said}"
    );
}
