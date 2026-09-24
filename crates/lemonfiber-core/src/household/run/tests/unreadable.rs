//! What a service that will not answer costs the household view, and what it does not.

use super::*;

#[tokio::test]
async fn a_service_still_starting_costs_names_without_being_called_a_failed_read() {
    // No key is readable yet, so no library opens. That is a service still coming up
    // rather than one that refused, so it is skipped — the requests still report
    // where they stand, and nothing claims a read failed that was never made.
    let mut context = ctx_with(
        &Fake {
            sign_in: "",
            requests: r#"{"pageInfo":{"results":1},"results":[
                {"status":2,"type":"tv","media":{"status":5,"externalServiceId":1},
                 "requestedBy":{"displayName":"Alex"}}
            ]}"#,
            library: r#"[{"id":1,"title":"The Expanse","monitored":true}]"#,
            refuse: false,
            ..Fake::default()
        },
        "starting",
    );
    context = context.with_filesystem(Arc::new(SeedFs::keyed(None, None)));
    let report = household(&context, None).await.unwrap_or_default();
    assert!(report.available);
    let first = report.members.first().and_then(|m| m.requests.first());
    assert_eq!(first.and_then(|request| request.title.clone()), None);
    assert_eq!(first.and_then(|request| request.state), Some(State::Here));
    // Skipped, not failed: no unreadable-library finding is raised. Asserted on
    // the subject rather than on the count, because this fixture answers nothing
    // about what the household may *ask* for either, and that gap has a line of
    // its own — one whose arrival here would otherwise read as the library's.
    assert!(
        !report.findings.iter().any(|said| said.contains("librar")),
        "{report:?}"
    );
}

#[tokio::test]
async fn a_refused_sign_in_costs_the_requests_and_not_the_household() {
    // Who is in the house is the media server's fact. Reporting nobody because the
    // *request* service refused would be the same defect this read was built to
    // fix, one service along — so the members still list and the refusal is said.
    let context = ctx_with(
        &Fake {
            sign_in: "no",
            refuse: true,
            ..Fake::default()
        },
        "refused",
    );
    let report = household(&context, None).await.unwrap_or_default();
    assert!(report.available);
    assert_eq!(
        report
            .members
            .iter()
            .map(|member| member.name.as_str())
            .collect::<Vec<&str>>(),
        vec!["Alex"],
        "a refused request service emptied the household: {report:?}"
    );
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.contains("would not accept")),
        "{report:?}"
    );
}

#[tokio::test]
async fn an_unreadable_request_record_costs_the_requests_and_not_the_household() {
    let context = ctx_with(
        &Fake {
            sign_in: "",
            requests: "not json",
            library: "[]",
            refuse: false,
            ..Fake::default()
        },
        "unreadable",
    );
    let report = household(&context, None).await.unwrap_or_default();
    assert!(report.available);
    assert_eq!(
        report
            .members
            .iter()
            .map(|member| member.name.as_str())
            .collect::<Vec<&str>>(),
        vec!["Alex"],
        "an unreadable request record emptied the household: {report:?}"
    );
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.contains("could not be read")),
        "{report:?}"
    );
}

/// With nothing to sign in to the request service with, that is said and the
/// household still reads.
///
/// Driven at `reaching` directly: reached through the whole command, the media
/// server's own reader refuses first for the same missing password, so the branch
/// this is about is never the one that answers.
#[tokio::test]
async fn with_nothing_to_ask_the_request_service_with_the_requests_are_what_is_lost() {
    let context = a_context().build();
    let services = context
        .stack
        .checked_manifest(context.today())
        .map(|manifest| manifest.services)
        .unwrap_or_default();
    assert!(
        !services.is_empty(),
        "the shipped stack declared no services, so this asserts nothing"
    );

    let asked = reaching(&context, &services).await;

    assert!(
        asked.is_err_and(|reason| reason.contains("no request service")),
        "a stack with nothing to sign in with did not say so"
    );
}

/// A media server that will not say who holds an account is unavailable, not empty.
///
/// The one refusal that *does* blank the list, because the accounts are where the
/// household comes from — and it is said rather than shown as a house with nobody
/// in it, which is the reading an empty list would invite.
#[tokio::test]
async fn a_media_server_that_will_not_say_who_is_here_is_unavailable_not_empty() {
    let context = ctx_with(
        &Fake {
            accounts: "not json",
            ..Fake::default()
        },
        "unreadable-accounts",
    );

    let report = household(&context, None).await.unwrap_or_default();

    assert!(!report.available, "{report:?}");
    assert!(report.members.is_empty(), "{report:?}");
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.contains("would not say who holds an account")),
        "{report:?}"
    );
}

/// Libraries that will not read cost their names, not the access.
///
/// The member still reports what they may watch — the server said which libraries,
/// and only what they are *called* is missing, so the identifiers stand in and a
/// finding says why.
#[tokio::test]
async fn libraries_that_will_not_read_cost_their_names_and_not_the_access() {
    let context = ctx_with(
        &Fake {
            folders: "not json",
            ..Fake::default()
        },
        "unnamed-libraries",
    );

    let report = household(&context, None).await.unwrap_or_default();

    assert!(report.available, "{report:?}");
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.contains("libraries could not be read")),
        "{report:?}"
    );
}

#[tokio::test]
async fn an_unreadable_library_costs_names_not_the_view() {
    let context = ctx_with(
        &Fake {
            sign_in: "",
            requests: r#"{"pageInfo":{"results":1},"results":[
                {"status":2,"type":"tv","media":{"status":5,"externalServiceId":1},
                 "requestedBy":{"displayName":"Alex"}}
            ]}"#,
            library: "not json",
            refuse: false,
            ..Fake::default()
        },
        "unnamed",
    );
    let report = household(&context, None).await.unwrap_or_default();
    // The request still reports where it stands; only its name is missing, and the
    // gap is said rather than left to look like an item with no title.
    assert!(report.available);
    let first = report.members.first().and_then(|m| m.requests.first());
    assert_eq!(first.and_then(|request| request.title.clone()), None);
    assert_eq!(first.and_then(|request| request.state), Some(State::Here));
    assert!(report
        .findings
        .iter()
        .any(|finding| finding.contains("library could not be read")));
}

#[tokio::test]
async fn a_stack_with_no_recorded_password_has_nothing_to_ask_with() {
    // No env file, so no recorded media-server password — there is no account to
    // sign in as, which is said rather than shown as a household that asked for
    // nothing.
    let context = a_context()
        .build()
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)))
        .with_http(
            Fake {
                sign_in: "",
                requests: "",
                library: "[]",
                refuse: false,
                ..Fake::default()
            }
            .transport(),
        );
    let report = household(&context, None).await.unwrap_or_default();
    assert!(!report.available);
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.contains("no recorded")),
        "{report:?}"
    );
}

/// Ratings that will not read cost the certificates, not the limit.
///
/// The words for the number still read and lemonfiber's own mapping stands in for
/// the names — which is a claim about where those names came from, so it is said
/// rather than left for a parent to take as their own server's.
#[tokio::test]
async fn ratings_that_will_not_read_cost_the_certificates_and_not_the_limit() {
    let context = ctx_with(
        &Fake {
            ratings: "not json",
            accounts: r#"[{"Id":"a1","Name":"Alex","HasPassword":true,
                "Policy":{"EnableAllFolders":true,"MaxParentalRating":12}}]"#,
            ..Fake::default()
        },
        "unreadable-ratings",
    );

    let report = household(&context, None).await.unwrap_or_default();

    assert!(report.available, "{report:?}");
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.contains("own ratings could not be read")),
        "{report:?}"
    );
    let rated = report
        .members
        .first()
        .and_then(|member| member.access.rated.clone())
        .unwrap_or_default();
    assert!(rated.fell_back, "{rated:?}");
    assert_eq!(rated.allows, vec!["12A".to_owned()], "{rated:?}");
}

/// An account holding unrated content back reads as one that does.
///
/// The server keeps it as a list of kinds and what a household means by it is all
/// of them or none, so a list with anything in it is the one answer that question
/// has — and a member missing half the library is either this or a defect.
#[tokio::test]
async fn an_account_that_holds_unrated_content_back_reads_as_one_that_does() {
    let context = ctx_with(
        &Fake {
            accounts: r#"[{"Id":"a1","Name":"Alex","HasPassword":true,
                "Policy":{"EnableAllFolders":true,"MaxParentalRating":12,
                "BlockUnratedItems":["Movie","Series"]}}]"#,
            ..Fake::default()
        },
        "unrated-held",
    );

    let report = household(&context, None).await.unwrap_or_default();

    assert_eq!(
        report.members.first().map(|member| member.access.unrated),
        Some(crate::ports::service::Unrated::HeldBack),
        "{report:?}"
    );
}

#[tokio::test]
async fn a_household_view_over_an_unreadable_stack_is_an_error() {
    let mut context = ctx_with(
        &Fake {
            sign_in: "",
            requests: "",
            library: "[]",
            refuse: false,
            ..Fake::default()
        },
        "badstack",
    );
    context.stack = crate::stack::Source::External(std::path::Path::new("/nowhere/at/all"));
    assert!(household(&context, None).await.is_err());
}
