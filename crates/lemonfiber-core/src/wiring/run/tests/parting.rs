//! A choice of filler stops the part it passes over, journalled before the choice, and
//! undoing it starts that part again.

use std::sync::Arc;

use lemonfiber_fixtures::support::Recording;

use super::{agreed, ctx, fill};
use crate::journal::{Action, Kind};
use crate::ports::process::Output;

/// A context whose engine answers every command with `status`, and what it was handed.
fn engine(named: &str, status: i32) -> (crate::app::Ctx, Arc<Recording>) {
    let (mut ctx, _) = ctx(named);
    let runner = Arc::new(Recording::answering(Ok(Output {
        status: Some(status),
        stdout: String::new(),
        stderr: "busy".to_owned(),
    })));
    ctx.seams.runner = runner.clone();
    (ctx, runner)
}

/// Whether the engine was handed `verb` aimed at `service`.
fn handed(runner: &Recording, verb: &str, service: &str) -> bool {
    runner.seen().iter().any(|argv| {
        argv.iter().any(|word| word == verb) && argv.last().is_some_and(|last| last == service)
    })
}

#[tokio::test]
async fn a_choice_stops_the_part_it_passes_over_journalled_before_the_choice() {
    let (ctx, runner) = engine("parted", 0);

    let report = agreed(&ctx, fill("indexer.search", "nzbhydra2")).await.ok();

    assert_eq!(
        report
            .as_ref()
            .map(|report| (report.substitution.stops.clone(), report.unran.len())),
        Some((vec!["prowlarr".to_owned()], 0))
    );
    assert!(handed(&runner, "stop", "prowlarr"));
    let changes = crate::app::recover::journal_at(
        &crate::app::targets::layout(&ctx)
            .map(|paths| paths.journal())
            .unwrap_or_default(),
    )
    .map(|journal| journal.changes().to_vec())
    .unwrap_or_default();
    assert!(matches!(
        changes.first().map(|change| &change.kind),
        Some(Kind::Ran { service, running: false }) if service == "prowlarr"
    ));

    let stamp = changes
        .first()
        .map(|change| change.at.clone())
        .unwrap_or_default();
    let back = crate::app::putting_back::undo(&ctx, Some(&stamp))
        .await
        .ok();

    assert!(
        back.is_some_and(|back| back.reversed.iter().any(|undo| matches!(
            &undo.action,
            Action::Run { service, running: true } if service == "prowlarr"
        )))
    );
    assert!(handed(&runner, "up", "prowlarr"));
}

#[tokio::test]
async fn a_part_the_engine_will_not_stop_is_named_and_the_choice_stands() {
    let (ctx, _) = engine("unparted", 1);

    let report = agreed(&ctx, fill("indexer.search", "nzbhydra2")).await.ok();

    assert!(report.is_some_and(|report| report.applied
        && report
            .unran
            .iter()
            .any(|one| one.service == "prowlarr"
                && one.because == "the container engine refused: busy")));
}

#[tokio::test]
async fn choosing_the_part_back_starts_it_again() {
    let (ctx, runner) = engine("chosen-back", 0);
    let _ = agreed(&ctx, fill("indexer.search", "nzbhydra2")).await;

    let back = agreed(&ctx, fill("indexer.search", "prowlarr")).await.ok();

    assert_eq!(
        back.map(|back| (back.substitution.starts, back.substitution.stops)),
        Some((vec!["prowlarr".to_owned()], vec!["nzbhydra2".to_owned()]))
    );
    assert!(handed(&runner, "up", "prowlarr"));
}

#[cfg(unix)]
#[tokio::test]
async fn a_part_the_reversal_may_not_start_is_left_named() {
    let (ctx, runner) = engine("unstarted", 0);
    let _ = agreed(&ctx, fill("indexer.search", "nzbhydra2")).await;
    let stack = crate::app::project::directory(&ctx).unwrap_or_default();
    let elsewhere = stack.with_file_name("elsewhere.yml");
    let _ = std::fs::create_dir_all(&stack);
    let _ = std::fs::write(&elsewhere, "services: {}\n");
    let _ = std::fs::remove_file(stack.join(crate::project::FILE));
    let _ = std::os::unix::fs::symlink(&elsewhere, stack.join(crate::project::FILE));
    let stamp = crate::app::recover::journal_at(
        &crate::app::targets::layout(&ctx)
            .map(|paths| paths.journal())
            .unwrap_or_default(),
    )
    .ok()
    .and_then(|journal| journal.changes().first().map(|change| change.at.clone()))
    .unwrap_or_default();

    let back = crate::app::putting_back::undo(&ctx, Some(&stamp))
        .await
        .ok();

    assert!(back.is_some_and(|back| back.left.iter().any(|left| left.target == "prowlarr")));
    assert!(!handed(&runner, "up", "prowlarr"));
}

#[tokio::test]
async fn a_part_no_engine_could_be_run_for_is_named_with_why() {
    let (mut ctx, _) = ctx("no-engine");
    ctx.seams.runner = Arc::new(Recording::answering(Err(
        crate::ports::process::Failure::NotFound {
            program: "docker".to_owned(),
        },
    )));

    let report = agreed(&ctx, fill("indexer.search", "nzbhydra2")).await.ok();

    assert!(report.is_some_and(|report| report
        .unran
        .iter()
        .any(|one| one.service == "prowlarr" && one.because == "docker was not found")));
}
