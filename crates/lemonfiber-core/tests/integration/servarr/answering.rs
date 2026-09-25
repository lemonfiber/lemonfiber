//! Who a service says it is, and how its refusals are read.

use super::sonarr;
use lemonfiber_core::ports::http::Method;
use lemonfiber_core::ports::service::Failure;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::service::Maintenance;
use lemonfiber_ports::Client;

#[tokio::test]
async fn a_valid_credential_reads_the_service_identity() {
    let fake = Fake::always(Answer::reply(
        200,
        r#"{"instanceName":"Sonarr","appName":"Sonarr","version":"4.0.15.2941"}"#,
    ));
    let identity = sonarr(&fake).identity().await;
    assert_eq!(
        identity.ok().map(|who| (who.name, who.version)),
        Some(("Sonarr".to_owned(), "4.0.15.2941".to_owned()))
    );

    // The credential rode the right header to the right route.
    let sent = fake.request();
    assert!(sent
        .as_ref()
        .is_some_and(|request| request.url.ends_with("/api/v3/system/status")));
    assert!(sent.is_some_and(|request| request
        .headers
        .iter()
        .any(|(name, value)| name == "X-Api-Key" && value == "the-key")));
}

#[tokio::test]
async fn the_app_name_is_used_when_no_instance_name_is_set() {
    let fake = Fake::always(Answer::reply(
        200,
        r#"{"appName":"Radarr","version":"5.0"}"#,
    ));
    let identity = sonarr(&fake).identity().await;
    assert_eq!(identity.ok().map(|who| who.name), Some("Radarr".to_owned()));
}

#[tokio::test]
async fn a_command_is_posted_by_name_and_accepted() {
    let fake = Fake::always(Answer::reply(201, r#"{"name":"CutoffUnmetEpisodeSearch"}"#));
    let accepted = sonarr(&fake).run_command("CutoffUnmetEpisodeSearch").await;
    assert!(accepted.is_ok());

    // The command rode a POST to the command route, named in the body.
    let sent = fake.request();
    assert!(sent.as_ref().is_some_and(
        |request| request.method == Method::Post && request.url.ends_with("/api/v3/command")
    ));
    assert!(sent.is_some_and(|request| request
        .body
        .as_deref()
        .is_some_and(|body| body.contains("CutoffUnmetEpisodeSearch"))));
}

#[tokio::test]
async fn a_command_a_service_refuses_is_a_failure() {
    let fake = Fake::always(Answer::reply(500, "boom"));
    assert!(sonarr(&fake)
        .run_command("CutoffUnmetEpisodeSearch")
        .await
        .is_err());
}

#[tokio::test]
async fn a_rejected_key_is_unauthorised() {
    let fake = Fake::always(Answer::reply(401, ""));
    assert!(matches!(
        sonarr(&fake).identity().await,
        Err(Failure::Unauthorised { .. })
    ));
}

#[tokio::test]
async fn a_service_that_is_not_answering_is_unavailable() {
    let fake = Fake::always(Answer::Silent);
    assert!(matches!(
        sonarr(&fake).identity().await,
        Err(Failure::Unavailable { .. })
    ));
}

#[tokio::test]
async fn an_unexpected_status_is_refused_with_the_services_own_words() {
    // The service's own message is carried through, not paraphrased.
    let fake = Fake::always(Answer::reply(500, "database is locked"));
    let detail = match sonarr(&fake).identity().await {
        Err(Failure::Refused { detail, .. }) => Some(detail),
        _ => None,
    };
    assert!(
        detail.is_some_and(|words| words.contains("500") && words.contains("database is locked")),
        "the service's own words are carried through"
    );
}

#[tokio::test]
async fn an_unexpected_status_with_no_body_is_refused_with_its_code() {
    let fake = Fake::always(Answer::reply(503, ""));
    let detail = match sonarr(&fake).identity().await {
        Err(Failure::Refused { detail, .. }) => Some(detail),
        _ => None,
    };
    assert_eq!(detail.as_deref(), Some("HTTP 503"));
}

#[tokio::test]
async fn an_unreadable_status_body_is_refused_and_the_detail_names_the_break() {
    let fake = Fake::always(Answer::reply(200, "not json at all"));
    let detail = match sonarr(&fake).identity().await {
        Err(Failure::Refused { detail, .. }) => detail,
        _ => String::new(),
    };
    // The generic phrase, then the parser's own account of what failed — kept
    // rather than paraphrased, so a shape change is diagnosable.
    assert!(
        detail.starts_with("the status response could not be read: "),
        "missing the generic lead-in: {detail}"
    );
    assert!(
        detail.contains("expected") || detail.contains("column"),
        "the parser's own words should survive: {detail}"
    );
}

#[tokio::test]
async fn a_status_that_names_neither_itself_nor_its_version_is_refused() {
    let fake = Fake::always(Answer::reply(200, r#"{"version":"4.0"}"#));
    assert!(matches!(
        sonarr(&fake).identity().await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn a_service_that_does_not_serve_the_api_version_is_unsupported() {
    // A 404 is the whole /api/v{n} prefix not served — the service was upgraded
    // past (or stands before) the version this build speaks. Reported as
    // unsupported, naming the version, rather than read as a generic refusal so
    // seeding refuses it rather than writing something malformed.
    let fake = Fake::always(Answer::reply(404, ""));
    let detail = match sonarr(&fake).identity().await {
        Err(Failure::Unsupported { detail, .. }) => Some(detail),
        _ => None,
    };
    assert!(
        detail.is_some_and(|words| words.contains("/api/v3")),
        "the unsupported version is named"
    );
}

#[tokio::test]
async fn a_read_against_an_unsupported_api_version_is_unsupported_too() {
    // The seed read path shares the probe, so a 404 on the folder list is the same
    // unsupported-version signal — and it is the read, before any write, so nothing
    // malformed is ever posted.
    let fake = Fake::always(Answer::reply(404, ""));
    assert!(matches!(
        sonarr(&fake).root_folders().await,
        Err(Failure::Unsupported { .. })
    ));
}
