//! What an update finds before it moves anything.

use crate::{asking, behind, ctx, reported, Coming, Kept, Machine, SONARR};
use lemonfiber_core::app::{dispatch, Waiting};
use lemonfiber_core::update::State;

#[tokio::test]
async fn a_bare_run_names_both_versions_and_changes_nothing() {
    let machine = Machine::coming(Coming::Answering);
    let archive = Kept::writing(true);
    let context = ctx(&machine, behind(&[("sonarr", SONARR.0)]), &archive);

    let report = reported(dispatch(asking(false, Waiting::Never), &context).await);

    let read = report.map(|report| {
        (
            report.state,
            report.confirmed,
            report.changes.len(),
            report
                .changes
                .first()
                .map(|change| (change.current.clone(), change.target.clone())),
        )
    });
    assert_eq!(
        read,
        Some((
            State::UpdatesAvailable,
            false,
            1,
            Some((SONARR.0.to_owned(), SONARR.1.to_owned()))
        ))
    );
    assert_eq!(archive.written(), 0, "a bare run captured something");
    assert!(machine.started().is_empty(), "a bare run started something");
}

/// A stack with nothing to move asks the download clients nothing.
///
/// Going to two clients to establish that a stack already on its pins would
/// interrupt nothing is this command making work out of an answer of "nothing".
#[tokio::test]
async fn a_stack_on_every_pin_asks_the_download_clients_nothing() {
    let machine = Machine::coming(Coming::Answering);
    let archive = Kept::writing(true);
    let context = ctx(&machine, Vec::new(), &archive);

    let report = reported(dispatch(asking(false, Waiting::Never), &context).await);

    let read = report.map(|report| (report.state, report.in_flight.len(), report.confirmed));
    assert_eq!(read, Some((State::Current, 0, false)));
}

#[tokio::test]
async fn a_stack_already_on_its_pins_is_agreed_to_and_nothing_happens() {
    let machine = Machine::coming(Coming::Answering);
    let archive = Kept::writing(true);
    let context = ctx(&machine, Vec::new(), &archive);

    let report = reported(dispatch(asking(true, Waiting::Never), &context).await);

    let read = report.map(|report| (report.state, report.confirmed, report.backup));
    assert_eq!(read, Some((State::Current, true, None)));
    assert_eq!(archive.written(), 0, "nothing to move was captured anyway");
}

#[tokio::test]
async fn a_pin_older_than_what_is_running_is_reported_and_never_attempted() {
    let machine = Machine::coming(Coming::Answering);
    let archive = Kept::writing(true);
    // Standing on a version later than the pin, which is the one step lemonfiber
    // refuses: the database has been through it, and the older binary opening it is
    // what does the damage.
    let context = ctx(&machine, behind(&[("sonarr", "9.0.0")]), &archive);

    let report = reported(dispatch(asking(true, Waiting::Never), &context).await);

    let read = report.map(|report| {
        (
            report.changes.first().map(|change| change.refused),
            report.applied.len(),
        )
    });
    assert_eq!(read, Some((Some(true), 0)));
    assert!(machine.started().is_empty(), "a refused step was taken");
}
