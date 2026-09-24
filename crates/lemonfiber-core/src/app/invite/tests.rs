use super::{told, Linked};
use crate::test_support::a_context;

/// A stack with nothing to reach the request service with tells it nothing, and
/// says so as a thing not tried rather than a thing that failed.
///
/// Driven at `told` directly: reached through the whole command, the media
/// server's own reader refuses first for the same missing password, so the branch
/// this is about is never the one that answers.
///
/// Both halves say it, because both are about the same unreachable service: an
/// account it was never told about has nothing held on it either.
#[tokio::test]
async fn with_no_request_service_to_reach_nothing_is_tried() {
    let ctx = a_context().build();
    let services = ctx
        .stack
        .checked_manifest(ctx.today())
        .map(|manifest| manifest.services)
        .unwrap_or_default();
    assert!(
        !services.is_empty(),
        "the shipped stack declared no services, so this asserts nothing"
    );

    let said = told(&ctx, &services, &["1".to_owned()], Some("1")).await;

    assert_eq!(
        said.linked,
        Linked::NotTried,
        "a stack with nothing to sign in with reported a link that failed rather \
         than one nothing was tried on"
    );
    assert_eq!(said.requesting, Linked::NotTried);
}
