//! The stack's forms, services, checks and logs, read.

use crate::reading;
use crate::reading::*;

#[tokio::test]
async fn what_the_stack_declares_is_the_envelope_the_command_renders() {
    // The gap this read closed: the surface offers to start, stop and switch forms,
    // and until this endpoint existed a caller had to already know their names.
    let expected = as_the_command_renders_it(&world(running(), stack()), Command::Forms).await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(world(running(), stack()), "/api/forms").await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn the_forms_a_stack_declares_are_carried_in_their_own_envelope() {
    // Written out rather than derived, so a second serialisation could not pass
    // this by agreeing with itself.
    let seen = asked(world(running(), stack()), "/api/forms").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"forms","data":{"forms":[{"#)
            && body.contains(r#""id":"library""#)),
        "every form the stack declares, under the forms kind"
    );
}

#[tokio::test]
async fn naming_a_form_says_what_starting_it_would_come_to() {
    // One endpoint over two commands, because the command line spells the two with
    // one word: naming none lists them, naming some resolves them.
    let expected = as_the_command_renders_it(
        &world(running(), stack()),
        Command::Preview {
            forms: vec!["library".to_owned()],
        },
    )
    .await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(world(running(), stack()), "/api/forms?form=library").await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn a_form_that_is_named_is_not_answered_with_the_whole_list() {
    // The mistake the two commands exist to keep apart: a request that named a
    // form and was handed the catalogue would look like it had been answered.
    let seen = asked(world(running(), stack()), "/api/forms?form=library").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"preview""#)),
        "a named form is resolved rather than listed"
    );
}

#[tokio::test]
async fn what_the_stack_is_doing_is_the_envelope_the_command_renders() {
    // The whole of the contract in one assertion: the bytes a browser reads are
    // the bytes a script would have piped, produced by the same three calls.
    let expected = as_the_command_renders_it(
        &world(running(), stack()),
        Command::Ps { forms: Vec::new() },
    )
    .await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(world(running(), stack()), "/api/status").await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn what_the_stack_is_doing_is_carried_in_the_envelope_the_contract_states() {
    // Written out rather than derived, so a second serialisation could not pass
    // this by agreeing with itself.
    let seen = asked(world(running(), stack()), "/api/status").await;
    assert!(
        seen.is_some_and(
            |(_, body)| body.starts_with(r#"{"api_version":1,"kind":"status","data":{"forms":[],"#)
        ),
        "the envelope the whole stack is reported in"
    );
}

#[tokio::test]
async fn naming_a_form_narrows_what_the_services_read_reports() {
    let seen = asked(world(running(), stack()), "/api/services?form=library").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.contains(r#""forms":["library"]"#)
            && body.contains(r#""id":"jellyfin""#)),
        "a named form is the form reported on"
    );
}

#[tokio::test]
async fn naming_no_form_reports_on_the_whole_stack() {
    let seen = asked(world(running(), stack()), "/api/services").await;
    assert!(
        seen.is_some_and(
            |(status, body)| status == StatusCode::OK && body.contains(r#""forms":[],"#)
        ),
        "a read that narrows to nothing narrows to nothing"
    );
}

#[tokio::test]
async fn the_checks_answer_under_their_own_kind() {
    let seen = asked(world(running(), stack()), "/api/checks?only=vpn").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"doctor","data":{"#)),
        "a diagnosis is answered in a diagnosis's envelope"
    );
}

#[tokio::test]
async fn a_whole_diagnosis_is_what_a_read_naming_no_group_asks_for() {
    let seen = asked(world(running(), stack()), "/api/checks").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"doctor""#)),
        "naming no group runs every check there is"
    );
}

/// One check, asked for by the identifier its own finding carries.
#[tokio::test]
async fn a_single_check_is_asked_for_the_way_a_finding_names_it() {
    let seen = asked(
        world(running(), stack()),
        "/api/checks?only=environment.engine",
    )
    .await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.contains(r#""check":"environment.engine""#)
            && !body.contains(r#""check":"environment.compose""#)),
        "a read narrowed to one check answers with that check"
    );
}

#[tokio::test]
async fn a_group_of_checks_that_is_not_one_is_not_run() {
    // A name lemonfiber does not know is a mistake to correct, not a request to
    // answer with everything — the judgement the command line makes too.
    assert_eq!(
        asked(world(running(), stack()), "/api/checks?only=nonsense").await,
        Some((
            StatusCode::BAD_REQUEST,
            "There is no group of checks and no check by that name.".to_owned()
        ))
    );
}

#[tokio::test]
async fn the_disk_is_read_through_the_checks_that_are_about_it() {
    let seen = asked(world(running(), stack()), "/api/storage").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"doctor""#)
            && body.contains("storage")),
        "the disk's endpoint is the disk's group of checks"
    );
}

#[tokio::test]
async fn what_the_household_asked_for_is_answered_under_its_own_kind() {
    let seen = asked(world(running(), stack()), "/api/requests").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"household","data":{"#)),
        "the household's requests are answered in the household's envelope"
    );
}

#[tokio::test]
async fn naming_a_member_narrows_what_the_household_read_reports() {
    let seen = asked(world(running(), stack()), "/api/requests?member=ada").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"household""#)),
        "a named member is still the household's envelope"
    );
}

/// Naming none is the whole household, so an empty name cannot be read as naming
/// none. Left to reach the core it matched nobody and answered with a household
/// that has asked for nothing — the one reading this report is written to refuse.
#[tokio::test]
async fn a_member_given_and_left_empty_narrowed_to_nobody() {
    assert_eq!(
        asked(world(running(), stack()), "/api/requests?member=").await,
        Some((
            StatusCode::BAD_REQUEST,
            "Which member to narrow to must be named.".to_owned()
        ))
    );
}

#[tokio::test]
async fn what_the_services_said_arrives_as_one_envelope_a_line() {
    // A stream has no last element to close a document with, so the command line
    // emits an envelope a line and this answers with the same.
    let engine = Reporting::holding(&["sonarr"], Lifecycle::Running, Health::Healthy)
        .saying_at("sonarr", "2026-01-01T00:00:00Z", "started")
        .saying_at("sonarr", "2026-01-01T00:00:01Z", "importing");

    let seen = asked(world(engine, stack()), "/api/logs?service=sonarr&tail=10").await;
    assert_eq!(
        seen,
        Some((
            StatusCode::OK,
            concat!(
                r#"{"api_version":1,"kind":"log","data":{"service":"sonarr","stream":"stdout","#,
                r#""at":"2026-01-01T00:00:00Z","line":"started"}}"#,
                "\n",
                r#"{"api_version":1,"kind":"log","data":{"service":"sonarr","stream":"stdout","#,
                r#""at":"2026-01-01T00:00:01Z","line":"importing"}}"#,
                "\n",
            )
            .to_owned()
        ))
    );
}

#[tokio::test]
async fn a_service_with_nothing_to_say_answers_with_nothing() {
    // Not "no output": that sentence is for a person, and nobody is reading this.
    let engine = Reporting::holding(&["sonarr"], Lifecycle::Running, Health::Healthy);
    assert_eq!(
        asked(world(engine, stack()), "/api/logs").await,
        Some((StatusCode::OK, String::new()))
    );
}

#[tokio::test]
async fn a_form_narrows_a_log_read_the_way_it_narrows_the_command() {
    let engine = Reporting::holding(&["jellyfin"], Lifecycle::Running, Health::Healthy).saying_at(
        "jellyfin",
        "2026-01-01T00:00:00Z",
        "listening",
    );

    let seen = asked(world(engine, stack()), "/api/logs?form=library").await;
    assert!(
        seen.is_some_and(
            |(status, body)| status == StatusCode::OK && body.contains(r#""service":"jellyfin""#)
        ),
        "a form is the services it declares"
    );
}

#[tokio::test]
async fn a_line_count_that_is_not_a_number_is_refused() {
    assert_eq!(
        asked(world(running(), stack()), "/api/logs?tail=plenty").await,
        Some((
            StatusCode::BAD_REQUEST,
            "How many lines to begin with must be a number, and no more than 10000.".to_owned()
        ))
    );
}
