//! Versions, traces and what has stopped, read.

use super::reading::*;

#[tokio::test]
async fn the_versions_in_play_are_the_envelope_the_command_renders() {
    // The cheapest read there is: no arguments, and an answer the core already
    // renders for the command line.
    let expected = as_the_command_renders_it(&world(running(), stack()), Command::Version).await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(world(running(), stack()), "/api/version").await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn the_versions_in_play_are_carried_in_their_own_envelope() {
    // Written out rather than derived, so a second serialisation could not pass
    // this by agreeing with itself.
    let seen = asked(world(running(), stack()), "/api/version").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"version","data":{"binary":"#)),
        "the versions in play, under the version kind"
    );
}

#[tokio::test]
async fn following_one_item_is_the_envelope_the_command_renders() {
    let expected = as_the_command_renders_it(
        &world(running(), stack()),
        Command::Trace {
            term: "the expanse".to_owned(),
            season: None,
            searching: false,
        },
    )
    .await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(world(running(), stack()), "/api/trace?term=the+expanse").await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn the_term_a_trace_followed_is_the_term_that_was_asked_for() {
    // The whole request is its argument, so a read that dropped it would answer
    // about something else and look like it had answered.
    let seen = asked(world(running(), stack()), "/api/trace?term=the+expanse").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"trace","data":{"#)
            && body.contains(r#""item":"the expanse""#)),
        "the item followed is the one named"
    );
}

#[tokio::test]
async fn a_season_narrows_a_trace_the_way_it_narrows_the_command() {
    let expected = as_the_command_renders_it(
        &world(running(), stack()),
        Command::Trace {
            term: "the expanse".to_owned(),
            season: Some(2),
            searching: false,
        },
    )
    .await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(
            world(running(), stack()),
            "/api/trace?term=the+expanse&season=2"
        )
        .await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

#[tokio::test]
async fn a_trace_that_named_nothing_to_follow_is_refused() {
    // The command line requires the term too. A trace of everything is not a
    // smaller request than a trace of one thing; it is a different one.
    assert_eq!(
        asked(world(running(), stack()), "/api/trace").await,
        Some((
            StatusCode::BAD_REQUEST,
            "What to follow must be named.".to_owned()
        ))
    );
}

#[tokio::test]
async fn a_term_given_and_left_empty_named_nothing_to_follow() {
    assert_eq!(
        asked(world(running(), stack()), "/api/trace?term=").await,
        Some((
            StatusCode::BAD_REQUEST,
            "What to follow must be named.".to_owned()
        ))
    );
}

#[tokio::test]
async fn a_season_that_is_not_a_number_is_refused() {
    assert_eq!(
        asked(
            world(running(), stack()),
            "/api/trace?term=the+expanse&season=latest"
        )
        .await,
        Some((
            StatusCode::BAD_REQUEST,
            "Which season to narrow to must be a number.".to_owned()
        ))
    );
}

#[tokio::test]
async fn what_has_stopped_is_answered_under_its_own_kind() {
    // The landing point for the dashboard's own count of what is stuck, which
    // until this endpoint existed had nowhere on the web to go.
    let seen = asked(world(running(), stack()), "/api/stuck").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"stuck","data":{"items":"#)),
        "the stuck items are answered in the stuck envelope"
    );
}

#[tokio::test]
async fn the_one_address_for_the_household_is_answered_under_its_own_kind() {
    // The question a browser has no other way to ask: which one link to send. The
    // answer names the request service and names the index over every service as
    // something that is not a way in, rather than leaving a browser to decide.
    let household = Reporting::holding(
        &["jellyfin", "seerr", "homepage"],
        Lifecycle::Running,
        Health::Healthy,
    );
    // The machine is scripted: what this one is called differs on every machine the
    // tests run on.
    let here = world(household, stack()).with_site(Renamed::called(Some("kitchen-nas")));
    let seen = asked(here, "/api/front-door").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"front-door","data":{"#)
            && body.contains(r#""standing":"established""#)
            && body.contains(r#""service":"Seerr""#)
            && body.contains(r#""url":"http://kitchen-nas.local:5055","caution":null"#)
            && body.contains(r#""facing":"asking""#)
            && body.contains(r#""service":"Homepage","facing":"operators""#)),
        "the front door is answered in the front-door envelope"
    );
}

/// The survey a migration opens with, reachable without asking for anything to change.
#[tokio::test]
async fn what_is_already_here_is_answered_under_its_own_kind() {
    let seen = asked(world(running(), stack()), "/api/migration").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"migration","data":{"read":"#)),
        "what is already on this machine, under the migration kind"
    );
}

#[tokio::test]
async fn where_this_copy_stands_is_the_envelope_the_command_renders() {
    // A read and never a replacement. What a page is served is the line to copy for
    // whichever tool owns the copy that is running, and the whole point of serving
    // it rather than assembling it is that the line is the one a shell would print.
    let expected =
        as_the_command_renders_it(&world(running(), stack()), Command::SelfUpdate { to: None })
            .await;

    assert!(expected.is_some(), "the command answered");
    assert_eq!(
        asked(world(running(), stack()), "/api/update?what=self").await,
        expected.map(|body| (StatusCode::OK, body))
    );
}

/// The other object at the same door, and the half this surface never served before.
///
/// Named rather than defaulted: the two answers do not resemble each other, so a page
/// that asked about the stack and was handed the binary has been answered a question
/// it did not ask.
#[tokio::test]
async fn naming_the_stack_asks_about_the_services_rather_than_the_binary() {
    let seen = asked(world(running(), stack()), "/api/update?what=stack").await;

    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"update","data":{"state":"#)),
        "the stack's own update answers here, under its own kind"
    );
}

/// Neither object is the smaller case of the other, so naming none is refused.
///
/// This is where the read parts company with `/api/uninstall`, which has a reading
/// that removes nothing and can default to it.
#[tokio::test]
async fn naming_no_object_is_refused_rather_than_answered_with_either() {
    let seen = asked(world(running(), stack()), "/api/update").await;

    assert!(
        seen.is_some_and(|(status, _)| status != StatusCode::OK),
        "a request naming neither object was answered with one of them"
    );
}

#[tokio::test]
async fn naming_a_version_asks_this_read_about_that_one() {
    // The one parameter it takes, and the one question a downgrade asks. A browser
    // that named a version is answered about that version and about whether it reads
    // the configuration already on this machine.
    let seen = asked(world(running(), stack()), "/api/update?what=self&to=0.9.0").await;

    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::OK
            && body.starts_with(r#"{"api_version":1,"kind":"self-update","data":{"standing":"#)
            && body.contains(r#""asked":"0.9.0""#)
            && body.contains("0.9.0 is behind the copy running")),
        "a named version is asked about rather than dropped"
    );
}
