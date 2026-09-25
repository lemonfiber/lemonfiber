//! The capture taken before anything moves.

use super::{asking, behind, came_to, ctx, reported, Coming, Kept, Machine, SONARR};
use lemonfiber_core::app::{dispatch, Waiting};
use lemonfiber_core::ports::process::Failure as RunFailure;
use lemonfiber_core::update::{Ending, Reversal, State};

#[tokio::test]
async fn a_confirmed_run_captures_before_anything_opens_its_state_on_the_new_image() {
    let machine = Machine::coming(Coming::Answering);
    let archive = Kept::writing(true);
    let context = ctx(&machine, behind(&[("sonarr", SONARR.0)]), &archive);

    let report = reported(dispatch(asking(true, Waiting::Never), &context).await);

    let read = report.map(|report| {
        (
            report.state,
            report.backup.is_some(),
            came_to(&report.applied, "sonarr"),
            report.halted,
        )
    });
    assert_eq!(
        read,
        Some((
            State::Updated,
            true,
            Some((Ending::Updated, Reversal::Restore)),
            None
        ))
    );
    assert_eq!(archive.written(), 1);
    assert_eq!(machine.started(), vec!["sonarr".to_owned()]);
    // The capture took the whole stack down, so the run puts it back — everything it
    // did not move is on the version it was already running.
    assert!(
        machine
            .last()
            .ends_with(&["up".to_owned(), "--detach".to_owned()]),
        "the stack was left down: {:?}",
        machine.last()
    );
}

#[tokio::test]
async fn a_capture_that_will_not_write_stops_the_run_before_anything_moves() {
    let machine = Machine::coming(Coming::Answering);
    let archive = Kept::writing(false);
    let context = ctx(&machine, behind(&[("sonarr", SONARR.0)]), &archive);

    let refused = dispatch(asking(true, Waiting::Never), &context).await;

    assert!(refused.is_err(), "the update went ahead without a backup");
    assert!(
        machine.started().is_empty(),
        "a service was started with no archive to go back to"
    );
}

/// The capture is refused while anything might be writing, so the stack is stopped
/// before it is attempted — which means a capture that will not write leaves every
/// service down. That is not a thing the backup's own words would ever mention, and
/// it is the only part of this an operator has to act on straight away.
#[tokio::test]
async fn a_capture_that_will_not_write_says_the_stack_was_left_down() {
    let machine = Machine::coming(Coming::Answering);
    let archive = Kept::writing(false);
    let context = ctx(&machine, behind(&[("sonarr", SONARR.0)]), &archive);

    let refused = dispatch(asking(true, Waiting::Never), &context).await;

    let problem = refused.err();
    assert_eq!(
        problem.as_ref().map(|one| one.code.to_string()).as_deref(),
        Some("UPDATE-4"),
        "the operator was told the capture failed and not that the stack is down"
    );
    let remedies: Vec<String> = problem
        .as_ref()
        .map(|one| {
            one.remedies
                .iter()
                .filter_map(|remedy| remedy.detail.clone())
                .collect()
        })
        .unwrap_or_default();
    assert!(
        remedies
            .iter()
            .any(|detail| detail.contains("lemonfiber up")),
        "nothing offered to bring the stack back up: {remedies:?}"
    );
    assert!(
        problem.and_then(|one| one.cause).is_some(),
        "what stopped the capture was replaced rather than carried"
    );
}

/// A run where every step succeeded and the stack would not come back afterwards.
///
/// The update is done and there is nothing here to roll back. Reporting it as a
/// failure is what would have an operator reverse a database migration that worked,
/// which is the one move this whole feature exists to make unnecessary — so the
/// report survives, and where the stack was left is said in it.
#[tokio::test]
async fn a_stack_that_will_not_come_back_does_not_undo_the_update_it_reports() {
    let machine =
        Machine::coming(Coming::Answering).refusing_to_bring_it_back(Err(RunFailure::NotFound {
            program: "docker".to_owned(),
        }));
    let archive = Kept::writing(true);
    let context = ctx(&machine, behind(&[("sonarr", SONARR.0)]), &archive);

    let report = reported(dispatch(asking(true, Waiting::Never), &context).await);

    let read = report.as_ref().map(|report| report.state);
    assert_eq!(
        read,
        Some(State::Updated),
        "a run whose every step succeeded was reported as the update failing"
    );
    assert_eq!(
        machine.started(),
        vec!["sonarr".to_owned()],
        "the service moved and answered its probe"
    );
    assert!(
        report
            .as_ref()
            .and_then(|report| report.backup.as_ref())
            .is_some(),
        "the backup taken before anything moved was dropped with the error"
    );
    let halted = report.and_then(|report| report.halted);
    assert!(
        halted
            .as_deref()
            .is_some_and(|why| why.contains("lemonfiber up")),
        "nothing said the stack is down or how to bring it back: {halted:?}"
    );
}

#[tokio::test]
async fn a_stack_that_will_not_come_down_is_never_captured_and_never_moved() {
    // The capture is refused while anything might be writing, so a stop that could
    // not even be run is the end of the run rather than something to go on past.
    let machine =
        Machine::coming(Coming::Answering).refusing_the_stack(Err(RunFailure::NotFound {
            program: "docker".to_owned(),
        }));
    let archive = Kept::writing(true);
    let context = ctx(&machine, behind(&[("sonarr", SONARR.0)]), &archive);

    let refused = dispatch(asking(true, Waiting::Never), &context).await;

    assert!(
        refused.is_err(),
        "the run went on over a stack it could not stop"
    );
    assert_eq!(
        archive.written(),
        0,
        "a stack that never stopped was captured"
    );
    assert!(machine.started().is_empty(), "a service was moved anyway");
}
