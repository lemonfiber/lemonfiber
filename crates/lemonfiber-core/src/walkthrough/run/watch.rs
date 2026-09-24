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

use super::choose::Chosen;
use super::walk::Walk;
use crate::app::targets::{download_targets, project_directory, read_transfers};
use crate::app::Ctx;
use crate::ports::docker::LogQuery;
use crate::ports::service::{Added, Pipeline, QueueItem, TraceEvent};
use crate::trace::{Outcome, Stage};
use crate::walkthrough::{Line, Reason, Speed, Step};

/// How often the services are asked whether anything has moved. Slower than the engine's
/// own poll: this is a network round trip to two services, and a download does not change
/// meaningfully in half a second.
const POLL: std::time::Duration = std::time::Duration::from_secs(2);

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
    let query = LogQuery::last_words();
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
mod tests;
