//! The household's requests read from a request service a plugin brings, over its
//! contract.

use super::*;
use crate::test_support::{beside_contracted_requests, CONTRACTED_REQUESTS_AT};

/// The media server's household, and a request service speaking `request.intake` that
/// holds one request of Alex's.
fn over_the_contract() -> Arc<Transport> {
    Transport::by_path(vec![
        (
            "/Users/AuthenticateByName",
            Answer::reply(200, r#"{"AccessToken":"token"}"#),
        ),
        (
            "/Users",
            Answer::reply(
                200,
                r#"[{"Id":"a1","Name":"Alex","HasPassword":true,
                    "Policy":{"EnableAllFolders":true},
                    "LastActivityDate":"2026-08-30T10:00:00Z"}]"#,
            ),
        ),
        (
            "/lemonfiber/request.intake/v1/answers",
            Answer::reply(204, ""),
        ),
        (
            "/lemonfiber/request.intake/v1/requests",
            Answer::reply(200, r#"[{"id": 3, "member": "Alex"}]"#),
        ),
        ("/lemonfiber/request.intake/v1/", Answer::reply(503, "")),
        ("", Answer::reply(200, "[]")),
    ])
}

/// **What the household asked for is read from whatever fills `request.intake`.** A
/// plugin's request service speaking the contract is read over it at its own address,
/// and nothing is asked as the bundled request service.
#[tokio::test]
async fn a_request_service_speaking_the_contract_is_read_over_it() {
    let transport = over_the_contract();
    let ctx = beside_contracted_requests("household", true, Arc::clone(&transport));

    let report = household(&ctx, None).await.unwrap_or_default();

    let asked: Vec<(&str, usize)> = report
        .members
        .iter()
        .map(|member| (member.name.as_str(), member.requests.len()))
        .collect();
    assert_eq!(asked, vec![("Alex", 1)], "{report:?}");
    let urls: Vec<String> = transport
        .requests()
        .into_iter()
        .map(|one| one.url)
        .collect();
    assert!(
        urls.contains(&format!("{CONTRACTED_REQUESTS_AT}requests")),
        "{urls:?}"
    );
    assert!(!urls.iter().any(|url| url.contains("/api/v1/")), "{urls:?}");
}

/// A request service speaking the contract that cannot be asked over it is asked
/// nothing: the household is still read, without its requests.
#[tokio::test]
async fn a_request_service_speaking_the_contract_without_its_key_is_asked_nothing() {
    let transport = over_the_contract();
    let ctx = beside_contracted_requests("household-unkeyed", false, Arc::clone(&transport));

    let report = household(&ctx, None).await.unwrap_or_default();

    assert!(report.available, "{report:?}");
    assert!(
        report
            .members
            .iter()
            .all(|member| member.requests.is_empty()),
        "{report:?}"
    );
    assert!(
        !transport
            .requests()
            .iter()
            .any(|one| one.url.contains(":8080/") || one.url.contains("/api/v1/")),
        "{:?}",
        transport.requests()
    );
}
