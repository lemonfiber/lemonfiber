use lemonfiber_core::instant::Instant;
use lemonfiber_core::model::RunReport;
use lemonfiber_core::scheduled::kept::ScheduledRun;

use super::ran;

fn report(due: bool, ran: Option<ScheduledRun>, held: Option<&str>) -> RunReport {
    RunReport {
        service: "configarr".to_owned(),
        rehearsed: false,
        due,
        ran,
        skipped: 0,
        held: held.map(str::to_owned),
    }
}

fn run(exit: Option<i32>) -> ScheduledRun {
    ScheduledRun {
        started: Instant::from_seconds(1_759_400_000),
        ended: Instant::from_seconds(1_759_400_060),
        exit,
        log: vec!["synced 3 profiles".to_owned(), "refused: 401".to_owned()],
    }
}

#[test]
fn a_minute_nothing_fell_due_in_says_so() {
    let said = ran(&report(false, None, None)).text();
    assert!(
        said.contains("None of configarr's times fell due."),
        "{said}"
    );
    let said = crate::render::shaped(&lemonfiber_core::app::Outcome::Ran(report(
        true, None, None,
    )))
    .text();
    assert!(
        said.contains("configarr is due, and would run now."),
        "{said}"
    );
}

#[test]
fn a_look_held_back_says_why() {
    let said = ran(&report(false, None, Some("the stack is not running"))).text();
    assert!(
        said.contains("configarr was not looked at: the stack is not running."),
        "{said}"
    );
}

#[test]
fn a_run_that_succeeded_says_when_and_one_that_failed_says_its_last_lines() {
    let said = ran(&report(true, Some(run(Some(0))), None)).text();
    assert!(
        said.contains("configarr ran, from 2025-10-02T10:13:20 UTC"),
        "{said}"
    );
    assert!(!said.contains("refused: 401"), "{said}");

    let mut failed = report(true, Some(run(Some(2))), None);
    failed.skipped = 3;
    let said = ran(&failed).text();
    assert!(said.contains("configarr ran and exited with 2."), "{said}");
    assert!(said.contains("  refused: 401"), "{said}");
    assert!(said.contains("3 time(s) fell due while it ran"), "{said}");

    let said = ran(&report(true, Some(run(None)), None)).text();
    assert!(said.contains("configarr ended before it exited."), "{said}");
}
