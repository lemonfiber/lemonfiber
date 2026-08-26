//! Waiting on it, and knowing when to stop waiting.
//!
//! A first download is gigabytes. Watching some of it is the point — it is the proof that
//! something is actually happening — but holding an operator at a progress bar for forty
//! minutes is not a tutorial, it is a hostage situation. So the wait is bounded, and what
//! happens at the bound depends on what is true: a download that is moving is handed to
//! the background with its terminal given back, and one that is not is a diagnosis.
//!
//! Nothing here ever cancels anything. Whatever is in flight when the operator walks away
//! stays in flight — that is the promise the narration makes, and it is kept by simply
//! not acting on it.

use super::super::targets::{download_targets, project_directory, read_transfers};
use super::super::Ctx;
use super::choose::Chosen;
use super::walk::Walk;
use crate::ports::docker::LogQuery;
use crate::ports::service::{Added, Pipeline, QueueItem, TraceEvent};
use crate::trace::{Outcome, Stage};
use crate::walkthrough::{Line, Reason, Speed, Step};

/// How often the services are asked whether anything has moved. Slower than the engine's
/// own poll: this is a network round trip to two services, and a download does not change
/// meaningfully in half a second.
const POLL: std::time::Duration = std::time::Duration::from_secs(2);

/// How many recent lines are quoted from a service when something goes wrong.
const LAST_WORDS: u32 = 20;

/// How the wait ended.
pub(super) enum Landed {
    /// It reached the library on disk.
    Imported,
    /// It is still coming, and the operator has their terminal back.
    StillGoing,
    /// It stopped, for this reason.
    Stopped(Reason),
}

/// Wait for it to land, narrating what moves, for as long as that is reasonable.
pub(super) async fn watch(walk: &mut Walk<'_>, chosen: &Chosen<'_>, item: &Added) -> Landed {
    let arr = chosen.arr;
    // The operator's patience, which is one thing and already a knob: a run told to wait
    // less waits less here too, and a walkthrough is exactly the kind of run someone
    // scripting would want to bound.
    let deadline = walk.ctx.clock.now() + walk.ctx.patience;

    loop {
        let events = arr
            .service
            .item_history(chosen.kind(), item.id)
            .await
            .unwrap_or_default();
        let queue = arr
            .service
            .item_queue(chosen.kind(), item.id)
            .await
            .unwrap_or_default();

        if let Some(verdict) = settled(&events, &queue) {
            return verdict;
        }
        say_progress(walk, &events, &queue, &item.title).await;

        // Checked after the reading, so a walk with no patience at all still reports what
        // it saw rather than reporting nothing.
        if walk.ctx.clock.now() >= deadline {
            return past_patience(walk.furthest());
        }
        tokio::time::sleep(POLL).await;
    }
}

/// Whether the wait is over, either way.
fn settled(events: &[TraceEvent], queue: &[QueueItem]) -> Option<Landed> {
    if events
        .iter()
        .any(|event| event.outcome == Outcome::Imported)
    {
        return Some(Landed::Imported);
    }
    // A failed download that is no longer in the queue is not being retried; one still in
    // the queue is, and a retry in progress is not yet a failure to report.
    let failed = events
        .iter()
        .any(|event| event.outcome == Outcome::DownloadFailed);
    if failed && queue.is_empty() {
        return Some(Landed::Stopped(Reason::Stalled));
    }
    queue
        .iter()
        .any(|item| item.stuck)
        .then_some(Landed::Stopped(Reason::Stalled))
}

/// Say what has moved, where anything has.
async fn say_progress(
    walk: &mut Walk<'_>,
    events: &[TraceEvent],
    queue: &[QueueItem],
    title: &str,
) {
    let reached = furthest(events, queue);
    let step = Step::of_stage(reached);
    if step == Step::Downloading {
        // The client's own figures, where a client answers: a size and a rate are what
        // make a download look like progress rather than a hang.
        let speed = speed_of(walk.ctx, title).await;
        walk.say_if_new(speed.map_or_else(|| Line::at(step), Speed::line));
        return;
    }
    walk.say_if_new(Line::at(step));
}

/// The furthest stage the item has actually reached.
fn furthest(events: &[TraceEvent], queue: &[QueueItem]) -> Stage {
    let from_history = events
        .iter()
        .filter_map(|event| event.outcome.stage())
        .max()
        .unwrap_or_default();
    let from_queue = queue
        .iter()
        .map(|item| item.stage)
        .max()
        .unwrap_or_default();
    from_history.max(from_queue)
}

/// What the download clients say about this item, where one of them is carrying it.
async fn speed_of(ctx: &Ctx, title: &str) -> Option<Speed> {
    let manifest = ctx.stack.checked_manifest(ctx.today()).ok()?;
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let needle = first_word(title).to_lowercase();
    for target in download_targets(&manifest.services, project.as_deref()) {
        let carrying = read_transfers(ctx, &target)
            .await
            .into_iter()
            .find(|download| download.name.to_lowercase().contains(&needle));
        if let Some(download) = carrying {
            let left = download.remaining.unwrap_or_default();
            return Some(Speed {
                // What is left plus what is done, which is the only total either client
                // offers — neither reports the release's own size directly.
                total: left + done(left, download.progress),
                left,
                rate: download.speed.unwrap_or_default(),
            });
        }
    }
    None
}

/// How much of a download of `left` remaining bytes at `progress` percent is already
/// done. A progress of a hundred leaves nothing to infer, and one of zero leaves the
/// total unknowable, so both read as nothing done rather than as a divide by zero.
const fn done(left: u64, progress: u8) -> u64 {
    if progress == 0 || progress >= 100 {
        return 0;
    }
    left * progress as u64 / (100 - progress as u64)
}

/// The first word of a title, which is what a release name and a library title reliably
/// share — everything after it is the release group's business.
fn first_word(title: &str) -> &str {
    title.split_whitespace().next().unwrap_or(title)
}

/// What to do when the operator's patience runs out, given how far it got.
///
/// Pure, and total over every step: the wait ends here for whatever reason, and each of
/// the seven places it could have got to means something different to the operator.
const fn past_patience(furthest: Step) -> Landed {
    match furthest {
        // Downloading when the bound was reached: it is working, it is just big. The
        // operator gets their terminal back and the download keeps running.
        Step::Downloading | Step::Importing | Step::Scanning | Step::Available => {
            Landed::StillGoing
        }
        // Never got as far as a download: something between the search and the client did
        // not happen, and waiting longer would not have changed it.
        Step::Choosing | Step::Searching | Step::Grabbing => Landed::Stopped(Reason::NotGrabbed),
    }
}

/// What the services were saying, for a stop to quote.
///
/// The explanation for a failed import is almost always in the \*arr's own output, and an
/// operator who has to go and find it has been handed a fault report rather than a
/// diagnosis. Lines mentioning the item come first; where none does, the most recent are
/// shown, because something is better than a silent failure.
///
/// Withheld as they are gathered, the same rule the same output takes when it becomes an
/// error's detail: these reach a terminal under "What sonarr was saying" and a browser as
/// a stopped walkthrough's `logs`, and a fix at either would leave the other quoting a key.
///
/// Withheld *before* the service's name is put in front, which is not an ordering anybody
/// gets to choose freely. What arrives is a name, a colon and the rest, which is the exact
/// shape of a setting — so a stack running a service called `authelia` or `keycloak` would
/// have had every line of its output replaced by a redaction, the marker word in its name
/// eating the sentence it introduced.
pub(super) async fn what_was_said(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    title: &str,
) -> Vec<String> {
    /// How many lines are worth putting under a failure before it stops being a
    /// diagnosis and starts being a log dump.
    const KEPT: usize = 5;

    let named: Vec<String> = services
        .iter()
        .filter(|service| matches!(service.api.as_ref().map(|api| api.kind), Some(kind) if is_servarr(kind)))
        .map(|service| service.id.clone())
        .collect();
    if named.is_empty() {
        return Vec::new();
    }
    let query = LogQuery {
        tail: LAST_WORDS,
        follow: false,
    };
    let Ok(mut lines) = ctx.engine.logs(&ctx.settings.project, &named, query).await else {
        return Vec::new();
    };

    let needle = first_word(title).to_lowercase();
    let (mut about_it, mut recent) = (Vec::new(), Vec::new());
    while let Some(line) = lines.recv().await {
        let said = format!(
            "{}: {}",
            line.service,
            crate::config::store::withheld_text(&line.line)
        );
        if said.to_lowercase().contains(&needle) {
            about_it.push(said);
        } else {
            recent.push(said);
        }
    }
    if about_it.is_empty() {
        about_it = recent;
    }
    about_it.split_off(about_it.len().saturating_sub(KEPT))
}

/// Whether an API kind is one of the library managers whose output explains an import.
const fn is_servarr(kind: lemonfiber_manifest::ApiKind) -> bool {
    matches!(kind, lemonfiber_manifest::ApiKind::Servarr)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use super::super::fixtures::{ctx_with, Fake, Ticking, CARRYING_IT};
    use super::{
        done, first_word, furthest, past_patience, settled, speed_of, what_was_said, Landed,
    };
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
        assert!(!said.contains(&secret), "{said}");
        // What is left has to still be the diagnosis: the service that wrote it, the
        // item it is about, and the reason. A rule that ate the sentence would leave an
        // operator a stop with nothing under it.
        assert!(
            said.starts_with("sonarr: Sintel: import refused,"),
            "{said}"
        );
        assert!(said.ends_with("was rejected"), "{said}");
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
}
