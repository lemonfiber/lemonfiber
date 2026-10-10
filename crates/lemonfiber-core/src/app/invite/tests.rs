use lemonfiber_fixtures::http::{Answer, Fake};

use super::{told, Linked};
use crate::test_support::{a_context, beside_contracted_requests, CONTRACTED_REQUESTS_AT};

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
    let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) else {
        unreachable!("the shipped stack does not read");
    };
    assert!(
        !manifest.services.is_empty(),
        "the shipped stack declared no services, so this asserts nothing"
    );

    let said = told(&ctx, &manifest, &["1".to_owned()], Some("1")).await;

    assert_eq!(
        said.linked,
        Linked::NotTried,
        "a stack with nothing to sign in with reported a link that failed rather \
         than one nothing was tried on"
    );
    assert_eq!(said.requesting, Linked::NotTried);
}

/// What a request service speaking `request.intake` answers about the household.
fn intake() -> std::sync::Arc<Fake> {
    Fake::by_path(vec![
        ("/answers", Answer::reply(204, "")),
        ("/link_members", Answer::reply(204, "")),
        (
            "/requesting",
            Answer::reply(200, r#"{"id": "7", "approves_own": true}"#),
        ),
        ("/approval_first", Answer::reply(204, "")),
    ])
}

/// **A request service a plugin brings is told about the household over its contract.**
/// Every member is linked and the narrowed one held over `request.intake`, at the
/// plugin's own address, and nothing is asked as the bundled request service.
#[tokio::test]
async fn a_request_service_speaking_the_contract_is_told_over_it() {
    let http = intake();
    let ctx = beside_contracted_requests("invite", true, http.clone());
    let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) else {
        unreachable!("the stack without its request service does not read");
    };

    let said = told(&ctx, &manifest, &["1".to_owned()], Some("1")).await;

    assert_eq!(said.linked, Linked::Made);
    assert_eq!(said.requesting, Linked::Made);
    let asked: Vec<(String, Option<String>)> = http
        .requests()
        .into_iter()
        .map(|one| (one.url, one.body))
        .collect();
    assert!(
        asked.iter().any(
            |(url, body)| *url == format!("{CONTRACTED_REQUESTS_AT}link_members")
                && body.as_deref().is_some_and(|body| body.contains("\"1\""))
        ),
        "{asked:?}"
    );
    assert!(
        asked
            .iter()
            .any(|(url, _)| *url == format!("{CONTRACTED_REQUESTS_AT}approval_first")),
        "{asked:?}"
    );
    assert!(
        asked
            .iter()
            .all(|(url, _)| url.starts_with(CONTRACTED_REQUESTS_AT)),
        "{asked:?}"
    );
}

/// A request service speaking the contract that cannot be asked over it is told
/// nothing, and never as the bundled request service.
#[tokio::test]
async fn a_request_service_speaking_the_contract_without_its_key_is_told_nothing() {
    let http = intake();
    let ctx = beside_contracted_requests("invite-unkeyed", false, http.clone());
    let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) else {
        unreachable!("the stack without its request service does not read");
    };

    let said = told(&ctx, &manifest, &["1".to_owned()], Some("1")).await;

    assert_eq!(said.linked, Linked::NotTried);
    assert_eq!(said.requesting, Linked::NotTried);
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}
