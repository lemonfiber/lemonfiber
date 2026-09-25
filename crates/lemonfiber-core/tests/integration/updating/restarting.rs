//! Bringing each service back on its new image.

use super::{asking, behind, came_to, ctx, reported, Coming, Kept, Machine, SONARR};
use lemonfiber_core::app::{dispatch, Waiting};
use lemonfiber_core::ports::process::{Failure as RunFailure, Output};
use lemonfiber_core::update::{Ending, Reversal, State};
use std::time::Duration;

/// The same for Radarr, which the manifest declares after it.
const RADARR: (&str, &str) = ("5.13.0", "5.14.0");

#[tokio::test]
async fn a_service_that_does_not_come_back_halts_the_run_and_says_what_did_not_move() {
    let machine = Machine::coming(Coming::Never);
    let archive = Kept::writing(true);
    let context = ctx(
        &machine,
        behind(&[("sonarr", SONARR.0), ("radarr", RADARR.0)]),
        &archive,
    );

    let report = reported(dispatch(asking(true, Waiting::Never), &context).await);

    let read = report.map(|report| {
        (
            report.state,
            came_to(&report.applied, "sonarr"),
            came_to(&report.applied, "radarr"),
            report.halted.unwrap_or_default(),
        )
    });
    assert!(
        matches!(
            &read,
            Some((
                State::Failed,
                Some((Ending::NotStarted, Reversal::Restore)),
                Some((Ending::NotReached, Reversal::Rollback)),
                said,
            )) if said.contains("sonarr") && said.contains("lemonfiber up")
        ),
        "{read:?}"
    );
    assert_eq!(
        machine.started(),
        vec!["sonarr".to_owned()],
        "the run went on into the rest of the stack"
    );
    assert!(
        !machine
            .last()
            .ends_with(&["up".to_owned(), "--detach".to_owned()]),
        "a halted run started the rest of the stack anyway: {:?}",
        machine.last()
    );
}

#[tokio::test]
async fn a_service_that_comes_back_unwell_is_told_apart_from_one_that_never_came_back() {
    let machine = Machine::coming(Coming::Unwell);
    let archive = Kept::writing(true);
    let context = ctx(&machine, behind(&[("sonarr", SONARR.0)]), &archive);

    let report = reported(dispatch(asking(true, Waiting::Never), &context).await);

    let read = report.map(|report| {
        (
            report.state,
            report
                .applied
                .first()
                .and_then(|one| one.detail.clone())
                .unwrap_or_default(),
        )
    });
    assert!(
        matches!(&read, Some((State::Failed, said)) if said.contains("not working")),
        "{read:?}"
    );
}

#[tokio::test]
async fn a_service_still_starting_is_asked_again_rather_than_written_off() {
    // Unsettled on the first two listings and answering on the third, which is what a
    // service that is genuinely starting looks like. Patience is the run's own, so the
    // wait has somewhere to go rather than expiring on the first look.
    let machine = Machine::coming(Coming::Slowly(2));
    let archive = Kept::writing(true);
    let context = ctx(&machine, behind(&[("sonarr", SONARR.0)]), &archive)
        .with_patience(Duration::from_secs(600));

    let report = reported(dispatch(asking(true, Waiting::Never), &context).await);

    let read = report.map(|report| (report.state, came_to(&report.applied, "sonarr")));
    assert_eq!(
        read,
        Some((State::Updated, Some((Ending::Updated, Reversal::Restore)))),
        "waiting is the point: the answer changed while it waited"
    );
}

#[tokio::test]
async fn an_engine_that_stops_answering_mid_run_is_reported_rather_than_waited_out() {
    let machine = Machine::coming(Coming::Silent);
    let archive = Kept::writing(true);
    let context = ctx(&machine, behind(&[("sonarr", SONARR.0)]), &archive);

    let report = reported(dispatch(asking(true, Waiting::Never), &context).await);

    let said = report
        .and_then(|report| report.applied.first().and_then(|one| one.detail.clone()))
        .unwrap_or_default();
    assert!(said.contains("stopped answering"), "{said}");
}

#[tokio::test]
async fn a_start_that_could_not_be_run_leaves_the_service_where_it_was() {
    let machine = Machine::coming(Coming::Answering).refusing(Err(RunFailure::NotFound {
        program: "docker".to_owned(),
    }));
    let archive = Kept::writing(true);
    let context = ctx(&machine, behind(&[("sonarr", SONARR.0)]), &archive);

    let report = reported(dispatch(asking(true, Waiting::Never), &context).await);

    let read = report.map(|report| {
        (
            came_to(&report.applied, "sonarr"),
            report
                .applied
                .first()
                .and_then(|one| one.detail.clone())
                .unwrap_or_default(),
        )
    });
    assert!(
        matches!(
            &read,
            Some((Some((Ending::NotFetched, Reversal::Rollback)), said)) if said.contains("docker")
        ),
        "{read:?}"
    );
}

#[tokio::test]
async fn a_compose_that_refuses_a_start_is_reported_in_composes_own_words() {
    let machine = Machine::coming(Coming::Answering).refusing(Ok(Output {
        status: Some(1),
        stdout: String::new(),
        stderr: "no such image".to_owned(),
    }));
    let archive = Kept::writing(true);
    let context = ctx(&machine, behind(&[("sonarr", SONARR.0)]), &archive);

    let report = reported(dispatch(asking(true, Waiting::Never), &context).await);

    let said = report
        .and_then(|report| report.applied.first().and_then(|one| one.detail.clone()))
        .unwrap_or_default();
    assert_eq!(said, "no such image");
}

#[tokio::test]
async fn a_refusal_that_went_to_the_other_stream_is_still_the_operators_only_account() {
    let machine = Machine::coming(Coming::Answering).refusing(Ok(Output {
        status: Some(1),
        stdout: "the compose file names no such service".to_owned(),
        stderr: String::new(),
    }));
    let archive = Kept::writing(true);
    let context = ctx(&machine, behind(&[("sonarr", SONARR.0)]), &archive);

    let report = reported(dispatch(asking(true, Waiting::Never), &context).await);

    let said = report
        .and_then(|report| report.applied.first().and_then(|one| one.detail.clone()))
        .unwrap_or_default();
    assert_eq!(said, "the compose file names no such service");
}
