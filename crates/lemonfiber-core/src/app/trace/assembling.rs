//! Fragments into one view of where an item got to.
//!
//! The pure half: given what each service said, how far did it get, and which parts of it
//! got there. Nothing here reaches a service.

use crate::model::{TraceMoment, TraceReport, TraceStage};
use crate::ports::service::{ItemPart, QueueItem, TraceEvent};
use crate::trace::{Confidence, Coverage, Outcome, Part, Presence, Stage};

use super::explaining::{stall_reason, trace_findings};
use super::reading::Fragments;

/// Build the trace from what one \*arr knows and what the media server confirms: the
/// stages its history records, what its queue is doing now, whether it is finally in the
/// library, the furthest reached, and — where a record proves it — why it stopped.
pub(crate) fn assemble(
    service: &str,
    title: &str,
    monitored: bool,
    fragments: Fragments,
) -> TraceReport {
    let Fragments {
        events,
        queue,
        parts,
        library,
        reads,
    } = fragments;
    // Presence in the media server only means something for availability once an \*arr is
    // monitoring the item; for one nobody asked for, "not monitored" is the whole answer,
    // and a library match is not availability but a disagreement — surfaced below as a
    // finding, never folded into how far the item got.
    let unmanaged_but_present = !monitored && library == Some(Presence::Present);
    let library = monitored.then_some(library).flatten();

    // The queue holds one record per part, so the item as a whole is the furthest any of
    // them reached and stuck if any one of them is. The per-part detail is kept for the
    // coverage below, where it is what tells a download in flight from a grab gone quiet.
    let queue_stage = queue.iter().map(|record| record.stage).max();
    let queue_stuck = queue.iter().any(|record| record.stuck);

    // The stages the history advances the item through — a grab, an import. A failed
    // download or a removal is history to show but advances no stage, so it is left out of
    // how far the item got.
    let advancing: Vec<Stage> = events
        .iter()
        .filter_map(|event| event.outcome.stage())
        .collect();
    let max_history = advancing.iter().copied().max();
    // Built while the events are still to hand: a part is placed by its own history and
    // queue records, not by the item-wide stages those collapse into.
    //
    // Only an item made of parts has coverage to report. A film is the whole item, and one
    // whose parts could not be read reports that as a finding rather than as a series with
    // nothing in it.
    let coverage = (!parts.is_empty()).then(|| coverage_of(parts, &queue, &events));
    let mut reached = advancing;
    reached.extend(queue_stage);
    // The library is the last word on how far an item got: confirmed present, it is
    // available whatever the \*arr's own record stops at.
    let present = library == Some(Presence::Present);
    if present {
        reached.push(Stage::Available);
    }
    let furthest = Stage::furthest(monitored, &reached);

    let mut stages = Vec::new();
    if monitored {
        stages.push(TraceStage {
            stage: Stage::Monitored,
            service: service.to_owned(),
            at: None,
        });
    }
    // The reader gives history newest-first; a trace reads oldest-first, the order things
    // happened — building both the stages it advanced through and the full log of what was
    // tried, so a repeated grab or a download that failed is seen rather than flattened
    // into the single furthest stage.
    let mut history = Vec::new();
    for event in events.into_iter().rev() {
        if let Some(stage) = event.outcome.stage() {
            stages.push(TraceStage {
                stage,
                service: service.to_owned(),
                at: Some(event.at.clone()),
            });
        }
        history.push(TraceMoment {
            outcome: event.outcome,
            at: event.at,
        });
    }
    // The queue is the live state; add it only where it carries the item past what its
    // history already shows, so it is the current step rather than a repeat.
    if let Some(stage) = queue_stage {
        if max_history.is_none_or(|reached| stage > reached) {
            stages.push(TraceStage {
                stage,
                service: service.to_owned(),
                at: None,
            });
        }
    }
    // The media server's confirmation is the final stage — a present fact, so untimed, and
    // always past what a \*arr's history and queue can show.
    if present {
        stages.push(TraceStage {
            stage: Stage::Available,
            service: "Jellyfin".to_owned(),
            at: None,
        });
    }

    TraceReport {
        item: title.to_owned(),
        matched: monitored,
        furthest,
        stall: stall_reason(furthest, queue_stuck, library, reads),
        stages,
        history,
        coverage,
        findings: trace_findings(unmanaged_but_present, reads),
        // A presence found by matching titles across to the media server — the two ends
        // share no id — may not be the item asked for, so it is marked, never claimed.
        confidence: if present {
            Confidence::Uncertain
        } else {
            Confidence::Certain
        },
    }
}

/// Aggregate an item's parts into per-season coverage, each part's resting stage lifted by
/// what the queue is doing with it now.
///
/// The lift is the point: a part the service records as grabbed and nothing more has, on
/// its own record, been handed to a download client that never took it — but a queue record
/// for that same part says it is downloading right now. Without the join, every episode in
/// flight would read as a stalled grab, which is the one reading a trace exists to prevent.
pub(crate) fn coverage_of(
    parts: Vec<ItemPart>,
    queue: &[QueueItem],
    events: &[TraceEvent],
) -> Coverage {
    Coverage::of(
        parts
            .into_iter()
            .map(|part| Part {
                stage: part_stage(&part, queue, events),
                season: part.season,
                number: part.number,
                title: part.title,
            })
            .collect(),
    )
}

/// How far one part got: where the service's current record puts it, lifted by what the
/// queue holds for it now and what its history proves was tried.
///
/// A file on disk settles it — the file is the fact, and an import recorded in a history
/// the file no longer backs is stale news rather than a part that is here.
///
/// Otherwise a live queue record lifts any part, since a download under way is a fact
/// whoever is monitoring it, while a grab from the history lifts only a part someone is
/// still asking for: an old grab against a part nobody monitors explains nothing worth
/// chasing. The grab has to come from the history because the episode listing carries no
/// such flag — the one it defines is never populated there.
pub(crate) fn part_stage(part: &ItemPart, queue: &[QueueItem], events: &[TraceEvent]) -> Stage {
    let resting = Stage::of_part(part.monitored, part.has_file);
    if resting == Stage::Imported {
        return resting;
    }
    let mut stage = resting;
    if let Some(live) = queue
        .iter()
        .filter(|record| record.part == Some(part.id))
        .map(|record| record.stage)
        .max()
    {
        stage = stage.max(live);
    }
    let attempted = events
        .iter()
        .any(|event| event.part == Some(part.id) && event.outcome == Outcome::Grabbed);
    if resting == Stage::Monitored && attempted {
        stage = stage.max(Stage::Grabbed);
    }
    stage
}
