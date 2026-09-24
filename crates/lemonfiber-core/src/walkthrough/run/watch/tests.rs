use std::sync::Arc;
use std::time::Duration;

use super::super::fixtures::{ctx_with, Fake, Ticking, CARRYING_IT};
use super::{done, first_word, furthest, past_patience, settled, speed_of, what_was_said, Landed};
use crate::ports::docker::{Health, Lifecycle};
use crate::ports::service::{QueueItem, TraceEvent};
use crate::test_support::Reporting;
use crate::trace::{Outcome, Stage};
use crate::walkthrough::{Reason, Step};

/// A history event of one outcome.
fn event(outcome: Outcome) -> TraceEvent {
    TraceEvent {
        outcome,
        at: "2026-08-08T00:00:00Z".to_owned(),
        part: None,
    }
}

/// A queue record at one stage.
const fn queued(stage: Stage, stuck: bool) -> QueueItem {
    QueueItem {
        part: None,
        stage,
        stuck,
    }
}

/// How a verdict reads, so the three are compared rather than matched — a `matches!`
/// inside an `assert!` leaves a branch nothing ever takes.
fn named(landed: Option<&Landed>) -> &'static str {
    match landed {
        None => "still waiting",
        Some(Landed::Imported) => "imported",
        Some(Landed::StillGoing) => "still going",
        Some(Landed::Stopped(reason)) => reason.remedy(),
    }
}

#[test]
fn an_import_ends_the_wait() {
    assert_eq!(
        named(settled(&[event(Outcome::Imported)], &[]).as_ref()),
        "imported"
    );
}

#[test]
fn where_the_wait_ran_out_decides_what_it_meant() {
    // The same bound means two entirely different things depending on what had
    // happened by the time it was reached.
    for step in [
        Step::Downloading,
        Step::Importing,
        Step::Scanning,
        Step::Available,
    ] {
        assert_eq!(named(Some(&past_patience(step))), "still going", "{step:?}");
    }
    for step in [Step::Choosing, Step::Searching, Step::Grabbing] {
        assert_eq!(
            named(Some(&past_patience(step))),
            Reason::NotGrabbed.remedy(),
            "{step:?}"
        );
    }
}

#[test]
fn a_failed_download_still_in_the_queue_is_a_retry_and_not_yet_a_failure() {
    // Reporting the first failure of three attempts as the ending would be a
    // diagnosis of something that went on to work.
    let retrying = settled(
        &[event(Outcome::DownloadFailed)],
        &[queued(Stage::Downloading, false)],
    );
    assert_eq!(named(retrying.as_ref()), "still waiting");

    let given_up = settled(&[event(Outcome::DownloadFailed)], &[]);
    assert_eq!(named(given_up.as_ref()), Reason::Stalled.remedy());
}

#[test]
fn a_stuck_queue_record_stops_the_wait() {
    let stuck = settled(&[], &[queued(Stage::Downloading, true)]);
    assert_eq!(named(stuck.as_ref()), Reason::Stalled.remedy());
    assert_eq!(
        named(settled(&[], &[queued(Stage::Downloading, false)]).as_ref()),
        "still waiting"
    );
}

#[test]
fn the_furthest_stage_is_the_furthest_of_both_accounts() {
    // The queue knows about work in progress and the history knows about work that
    // finished; taking either alone would make the walk appear to go backwards.
    assert_eq!(
        furthest(
            &[event(Outcome::Grabbed)],
            &[queued(Stage::Downloading, false)]
        ),
        Stage::Downloading
    );
    assert_eq!(furthest(&[event(Outcome::Grabbed)], &[]), Stage::Grabbed);
    assert_eq!(furthest(&[], &[]), Stage::NotMonitored);
    // A removal is history to show, never forward progress.
    assert_eq!(
        furthest(&[event(Outcome::Removed)], &[]),
        Stage::NotMonitored
    );
}

#[test]
fn a_total_is_inferred_from_what_is_left_and_how_far_along_it_is() {
    // Neither client reports the release's own size, so it is the one figure that has
    // to be worked out rather than read.
    assert_eq!(done(500, 50), 500, "half left means half done");
    assert_eq!(done(750, 25), 250);
    assert_eq!(done(100, 0), 0, "nothing to infer from");
    assert_eq!(done(0, 100), 0, "nothing left to infer from");
}

#[test]
fn a_title_is_matched_by_the_word_a_release_name_would_share() {
    assert_eq!(first_word("Tears of Steel (2012)"), "Tears");
    assert_eq!(first_word("Sintel"), "Sintel");
    assert_eq!(first_word(""), "");
}

/// The stack's own services, which is what a quote is gathered from.
fn services(ctx: &crate::app::Ctx) -> Vec<lemonfiber_manifest::Service> {
    ctx.stack
        .checked_manifest(ctx.today())
        .map(|manifest| manifest.services)
        .unwrap_or_default()
}

#[tokio::test]
async fn a_failure_is_quoted_with_the_lines_about_the_item_first() {
    // An operator who has to go and find the explanation has been handed a fault
    // report rather than a diagnosis.
    let mut ctx = ctx_with(&Fake::default());
    ctx.engine = Arc::new(
        Reporting::holding(&["sonarr"], Lifecycle::Running, Health::Healthy)
            .saying("sonarr", "something else entirely")
            .saying("sonarr", "Sintel: no files are eligible for import"),
    );
    let said = what_was_said(&ctx, &services(&ctx), "Sintel").await;
    assert_eq!(
        said,
        vec!["sonarr: Sintel: no files are eligible for import"]
    );
}

/// A stopped walkthrough quotes the \*arr's own output, and a \*arr that fails while
/// authenticating quotes the credential it failed with.
///
/// These lines are printed under "What sonarr was saying" and served as a stopped
/// walkthrough's `logs`, so the withholding is where they are gathered — a fix at
/// either surface would leave the other one publishing the key.
#[tokio::test]
async fn what_a_service_was_saying_is_quoted_with_no_credential_in_it() {
    // Assembled rather than written out, so no value that reads as one sits here.
    let secret = ["abcdef", "1234", "567890"].concat();
    let mut ctx = ctx_with(&Fake::default());
    ctx.engine = Arc::new(
        Reporting::holding(&["sonarr"], Lifecycle::Running, Health::Healthy).saying(
            "sonarr",
            &format!("Sintel: import refused, api_key={secret} was rejected"),
        ),
    );
    let said = what_was_said(&ctx, &services(&ctx), "Sintel")
        .await
        .join("");
    assert!(
        !said.contains(&secret),
        "the credential survived into what the service was quoted as saying"
    );
    // What is left has to still be the diagnosis: the service that wrote it, the
    // item it is about, and the reason. A rule that ate the sentence would leave an
    // operator a stop with nothing under it.
    assert!(
        said.starts_with("sonarr: Sintel: import refused,"),
        "the service, the item and the refusal went with the credential"
    );
    assert!(
        said.ends_with("was rejected"),
        "why the import was refused went with the credential"
    );
}

#[tokio::test]
async fn a_failure_with_nothing_said_about_the_item_quotes_what_there_is() {
    // Something is better than a silent failure: the recent output is where the
    // explanation usually is even when it does not name the item.
    let mut ctx = ctx_with(&Fake::default());
    ctx.engine = Arc::new(
        Reporting::holding(&["sonarr"], Lifecycle::Running, Health::Healthy)
            .saying("sonarr", "permission denied writing /data/media"),
    );
    let said = what_was_said(&ctx, &services(&ctx), "Sintel").await;
    assert_eq!(said, vec!["sonarr: permission denied writing /data/media"]);
}

#[tokio::test]
async fn nothing_to_ask_and_nothing_that_answers_are_both_simply_no_quote() {
    let ctx = ctx_with(&Fake::default());
    assert!(what_was_said(&ctx, &[], "Sintel").await.is_empty());
    // The default fixture's engine is absent, so the logs cannot be read at all.
    assert!(what_was_said(&ctx, &services(&ctx), "Sintel")
        .await
        .is_empty());
}

#[tokio::test]
async fn a_download_the_client_is_carrying_is_narrated_with_its_own_figures() {
    // A size and a rate are what make a download look like progress rather than a
    // hang, and they come from the client rather than being guessed.
    let ctx = ctx_with(&Fake {
        transfers: CARRYING_IT,
        ..Fake::default()
    });
    let speed = speed_of(&ctx, "Sintel").await;
    // Mebibytes, as the client reports them.
    assert_eq!(speed.map(|speed| speed.left), Some(1050 * 1024 * 1024));
    assert!(
        speed.is_some_and(|speed| speed.total > speed.left),
        "half done means the total is larger than what is left"
    );
    assert!(speed.is_some_and(|speed| speed.rate > 0));
}

#[tokio::test]
async fn a_client_carrying_nothing_of_ours_has_no_figures_to_offer() {
    let ctx = ctx_with(&Fake::default());
    assert_eq!(speed_of(&ctx, "Sintel").await, None);
}

#[tokio::test]
async fn a_stack_that_cannot_be_read_offers_no_figures_either() {
    let mut ctx = ctx_with(&Fake::default());
    ctx.stack = crate::stack::Source::External(std::path::Path::new("/not-a-stack"));
    assert_eq!(speed_of(&ctx, "Sintel").await, None);
}

#[tokio::test(start_paused = true)]
async fn a_wait_that_has_not_run_out_looks_again() {
    // The poll is the difference between watching a download and taking one reading
    // of it. Time is faked at both ends: tokio's, so nothing actually sleeps, and the
    // stack's, so the bound is reached deterministically.
    let mut ctx = ctx_with(&Fake {
        history: r#"{"records":[{"eventType":"grabbed","date":"2026-08-08T00:00:00Z"}]}"#,
        queue: r#"{"records":[{"seriesId":7,"movieId":7,"trackedDownloadState":"downloading","trackedDownloadStatus":"ok"}],"totalRecords":1}"#,
        ..Fake::default()
    });
    ctx.clock = Arc::new(Ticking::by(Duration::from_secs(1)));
    ctx.patience = Duration::from_secs(90);

    let heard = super::super::fixtures::Recording::default();
    let report = crate::app::walkthrough(&ctx, Some("Sintel"), &heard).await;
    assert!(
        report.is_ok_and(|report| report.in_background),
        "it looked more than once and then handed the download over"
    );
}
