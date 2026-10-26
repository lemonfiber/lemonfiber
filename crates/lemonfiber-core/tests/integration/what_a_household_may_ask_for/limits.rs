//! The household's limits, chosen through the dispatcher.

use super::common::household::answering;
use lemonfiber_core::app::{dispatch, Chosen, Command};
use lemonfiber_core::asking::Policy;

/// A choice that names a policy needing a limit, with none anywhere, is refused
/// before anything is written.
#[tokio::test]
async fn a_limit_that_was_never_named_is_refused_before_anything_is_written() {
    let refused = dispatch(
        Command::Allowing(Chosen {
            member: None,
            policy: Some(Policy::WithinALimit),
            quota: None,
        }),
        &answering("nolimit"),
    )
    .await;

    assert_eq!(
        refused.err().map(|problem| problem.code),
        Some(lemonfiber_core::error::codes::quota::NO_LIMIT)
    );
}
