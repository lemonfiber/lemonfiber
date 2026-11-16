use lemonfiber_fixtures::hosting::Hosting;

use super::{Check, ScheduledCheck, Verdict};
use crate::instant::Instant;
use crate::model::ScheduledService;
use crate::ports::hosting::{Cadence, Host, Hosted, Manager};
use crate::scheduled::kept::ScheduledRun;

fn ran(exit: Option<i32>) -> ScheduledRun {
    ScheduledRun {
        started: Instant::from_seconds(1_759_400_000),
        ended: Instant::from_seconds(1_759_400_060),
        exit,
        log: vec!["refused: 401".to_owned()],
    }
}

fn service(id: &str, last: Option<ScheduledRun>) -> ScheduledService {
    ScheduledService {
        id: id.to_owned(),
        name: id.to_owned(),
        running: false,
        fired: true,
        health: crate::model::RunHealth::of(last.as_ref()),
        last,
        skipped: 0,
    }
}

#[tokio::test]
async fn each_run_and_exit_service_is_a_finding_of_its_own_that_warns_where_its_last_run_failed() {
    let hosting = Hosting::with(Manager::Launchd);
    for id in ["run-fine", "run-new", "run-configarr"] {
        hosting
            .place(&job(id))
            .await
            .unwrap_or_else(|why| unreachable!("placed: {why:?}"));
    }
    let found = ScheduledCheck::new(
        vec![
            service("fine", Some(ran(Some(0)))),
            service("new", None),
            service("configarr", Some(ran(Some(2)))),
            service("unfired", None),
        ],
        hosting,
    )
    .run()
    .await;
    let [fine, new, failed, unfired] = found.as_slice() else {
        unreachable!("one finding each: {found:?}");
    };
    let Verdict::Warn(unfired) = &unfired.verdict else {
        unreachable!("a service nothing fires warns: {found:?}");
    };
    assert!(
        unfired
            .summary
            .contains("Nothing on this machine runs unfired"),
        "{unfired:?}"
    );
    assert_eq!(fine.check, "services.scheduled.fine");
    assert!(
        matches!(&fine.verdict, Verdict::Pass { note: Some(note) } if note.contains("last ran at")),
        "{found:?}"
    );
    assert!(
        matches!(&new.verdict, Verdict::Pass { note: Some(note) } if note.contains("not run yet")),
        "{found:?}"
    );
    let Verdict::Warn(problem) = &failed.verdict else {
        unreachable!("a failed run warns: {found:?}");
    };
    assert!(problem.summary.contains("exited with 2"), "{problem:?}");
    assert_eq!(problem.detail.as_deref(), Some("refused: 401"));
    assert_eq!(failed.category, super::Category::Services);
}

fn job(name: &str) -> Hosted {
    Hosted {
        name: name.to_owned(),
        program: "/usr/local/bin/lemonfiber".into(),
        arguments: Vec::new(),
        output: "/records/run.log".into(),
        about: String::new(),
        cadence: Cadence::Minutely,
    }
}

#[tokio::test]
async fn a_service_whose_job_is_held_against_a_program_now_gone_is_not_fired() {
    let hosting = Hosting::holding(Manager::Launchd, "run-gone", Hosting::orphaned("run-gone"));
    let found = ScheduledCheck::new(vec![service("gone", None)], hosting)
        .run()
        .await;
    assert!(
        matches!(found.as_slice(), [one] if matches!(one.verdict, Verdict::Warn(_))),
        "{found:?}"
    );
}
