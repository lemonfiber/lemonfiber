//! Watching the pipeline across the services that each see half of it.
//!
//! The \*arrs know what they are waiting for; the download clients know what they
//! are fetching. Neither knows what the other is doing, which is why the failure
//! that matters most — downloaded successfully, never imported — is invisible to
//! both and has to be assembled here.
//!
//! How long something has been wrong is the one thing neither side reports, and
//! it cannot be inferred from what they do: time since an item was added would
//! call a download added three days ago and stalled ten minutes ago "stalled for
//! three days". So the age comes from the condition store, which stamps a fault
//! when it is first seen and leaves the stamp alone while it persists. That also
//! gives a self-resolving stall its resolution for free: an item that recovers is
//! simply not raised on the next pass, and the store clears it.

use std::collections::BTreeMap;

use crate::condition::{Conditions, Fault};
use crate::error::Severity;
use crate::ports::service::Queued;
use crate::queue::{category, Fetching, Importing, Item, Stall, Stuck, Thresholds};

/// The check prefix a stuck item's condition is filed under.
pub const CHECK: &str = "queue";

/// What one service answered when asked for its queue.
pub enum Answered {
    /// It answered, with these items.
    Queue(Vec<Queued>),
    /// It could not be asked. Distinct from an empty queue: a service that did
    /// not answer says nothing about whether its queue is healthy, and reporting
    /// silence as an empty queue is how an operator comes to believe a pipeline is
    /// idle when it is unreachable.
    Unreachable,
}

/// What the pipeline amounts to across every service that was asked.
pub struct Watched {
    /// What is wrong, worst first.
    pub stuck: Vec<Stuck>,
    /// Services that could not be asked, by name — so an incomplete picture says
    /// so rather than passing for a complete one.
    pub unverified: Vec<String>,
}

impl Watched {
    /// A count per category, for a queue too long to list.
    ///
    /// Twenty stalled downloads are one sentence and one remedy; printing twenty
    /// lines is how a report stops being read at the point it starts mattering.
    #[must_use]
    pub fn by_category(&self) -> Vec<(Stall, usize)> {
        let mut counted: BTreeMap<Stall, usize> = BTreeMap::new();
        for stuck in &self.stuck {
            *counted.entry(stuck.stall).or_default() += 1;
        }
        counted.into_iter().collect()
    }
}

/// Assemble what the services said, record it, and report what has held long
/// enough to be worth saying.
///
/// `now` and the store together supply the age; the services supply everything
/// else. Every item looked at is recorded either way — a clean one clears its
/// condition, which is what makes a stall that resolved itself resolve in the
/// record too rather than lingering.
#[must_use]
pub fn watch(
    answers: &[(String, Answered)],
    fetching: &[(String, u8, bool)],
    conditions: &mut Conditions,
    thresholds: Thresholds,
    now: &str,
) -> Watched {
    let unverified: Vec<String> = answers
        .iter()
        .filter(|(_, answered)| matches!(answered, Answered::Unreachable))
        .map(|(service, _)| service.clone())
        .collect();

    let items = assemble(answers, fetching, conditions);
    let sayable: Vec<(&crate::queue::Item, Stall)> = items
        .iter()
        .filter_map(|item| sayable(item).map(|stall| (item, stall)))
        .collect();
    // What more than one item is blocked by. A full disk stops every download on
    // the machine, and twenty conditions about it are twenty alerts for one thing
    // to fix — which is how an operator learns to mute the queue check.
    let mut sharing: BTreeMap<&str, usize> = BTreeMap::new();
    for (item, _) in &sayable {
        if let Some(cause) = item.cause.as_deref() {
            *sharing.entry(cause).or_default() += 1;
        }
    }

    let mut stuck = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for (item, stall) in &sayable {
        let shared = item
            .cause
            .as_deref()
            .filter(|cause| sharing.get(cause).copied().unwrap_or(0) > 1);
        // Keyed by the cause where several share one, and otherwise by the item
        // *and* what is wrong with it — the age is per fault rather than per item,
        // since a download that stalled for a day and then began crawling has been
        // slow for a moment, not for a day.
        let check = shared.map_or_else(
            || format!("{CHECK}.{}.{}", kind_of(*stall), item.name),
            |cause| format!("{CHECK}.blocked.{cause}"),
        );
        let first = !seen.contains(&check);
        if first {
            seen.push(check.clone());
        }
        let group = shared.map(|cause| (cause, sharing.get(cause).copied().unwrap_or(1)));
        conditions.observe(&check, Some(&fault_for(item, *stall, group)), now);
        // The age is the store's: when this fault was first seen, not when the
        // item was added.
        let held = conditions
            .get(&check)
            .map(|condition| age(condition.since.as_str(), now))
            .unwrap_or_default();
        if first && !thresholds.within(thresholds.for_stall(*stall), held) {
            stuck.push(Stuck {
                name: shared.map_or_else(|| item.name.clone(), str::to_owned),
                stall: *stall,
                held_for: held.as_secs(),
                blocking: item.cause.clone(),
                items: shared.map_or(1, |cause| sharing.get(cause).copied().unwrap_or(1)),
            });
        }
    }
    // An item that left the pipeline entirely — imported, or removed — is the
    // commonest way a stall resolves itself, and it stops appearing rather than
    // appearing as fixed. Nothing above would have cleared it, so it would stand
    // raised for ever; the store is swept for anything under this prefix that was
    // not seen this pass.
    let gone: Vec<String> = conditions
        .all()
        .into_iter()
        .filter(|condition| condition.check.starts_with(&format!("{CHECK}.")))
        .filter(|condition| !seen.contains(&condition.check))
        .map(|condition| condition.check.clone())
        .collect();
    for check in gone {
        conditions.observe(&check, None, now);
    }

    stuck.sort_by(|left, right| {
        left.stall
            .cmp(&right.stall)
            .then_with(|| right.held_for.cmp(&left.held_for))
            .then_with(|| left.name.cmp(&right.name))
    });
    Watched { stuck, unverified }
}

/// What can honestly be said about one item, or nothing.
///
/// A finished download no \*arr is waiting for is either an orphan or a torrent
/// seeding after a successful import, and from here those are identical: the
/// queue holds what is in progress, so an imported item has left it. Telling them
/// apart needs the service's history, which nothing reads yet.
///
/// So nothing is said. Guessing orphan would flag every healthy seeding torrent
/// on the machine — the false positive the whole feature is built to avoid, and a
/// check that cries wolf about seeding is one the operator turns off. The
/// category stays in the model, reached once history is read.
fn sayable(item: &Item) -> Option<Stall> {
    match category(item) {
        Some(Stall::Orphaned) => None,
        other => other,
    }
}

/// How long ago a stamp was.
///
/// A stamp that cannot be read counts as no time at all, which delays a report
/// rather than inventing one — the safe direction for a clock nobody can trust.
fn age(since: &str, now: &str) -> std::time::Duration {
    let (since, now) = (
        since.parse::<u64>().unwrap_or_default(),
        now.parse::<u64>().unwrap_or_default(),
    );
    std::time::Duration::from_secs(now.saturating_sub(since))
}

/// One item per thing in the pipeline, holding both sides of it.
///
/// Correlated by name, which is what both sides call it. An item only one side
/// knows about is still an item — that absence is the signal in two of the
/// categories, so it must survive the join rather than being dropped by it.
fn assemble(
    answers: &[(String, Answered)],
    fetching: &[(String, u8, bool)],
    conditions: &Conditions,
) -> Vec<Item> {
    let mut by_name: BTreeMap<String, Item> = BTreeMap::new();

    for (name, progress, moving) in fetching {
        by_name
            .entry(name.clone())
            .or_insert_with(|| Item::named(name))
            .fetching = Some(Fetching {
            progress: *progress,
            moving: *moving,
        });
    }
    for queued in answers.iter().filter_map(|(_, answered)| match answered {
        Answered::Queue(items) => Some(items),
        Answered::Unreachable => None,
    }) {
        for item in queued {
            let held = by_name
                .entry(item.title.clone())
                .or_insert_with(|| Item::named(&item.title));
            // The service says it is failing, never how many times. The honest
            // count of repetition is the store's: how often this item's fault has
            // cleared and come back. Inventing a number here would be claiming to
            // have watched something nobody watched.
            let returned = conditions
                .all()
                .into_iter()
                .filter(|condition| condition.check.ends_with(&format!(".{}", item.title)))
                .map(|condition| condition.recurrences)
                .max()
                .unwrap_or(0);
            // Verbatim, and never interpreted here: what the service said is worth
            // more than any reading of it, and it is what tells one item's problem
            // from a condition stopping everything.
            held.cause.clone_from(&item.message);
            held.importing = Some(Importing {
                failures: if item.is_stuck() { returned } else { 0 },
                imported: false,
            });
            // The service's own count of how often it has fetched this since it
            // last imported it. The highest wins where two services claim the same
            // title, since one of them looping is a loop whatever the other says.
            held.grabs = held.grabs.max(item.grabs);
        }
    }
    by_name.into_values().collect()
}

/// The fault a stuck item raises, in the category's own words.
fn fault_for(item: &Item, stall: Stall, shared: Option<(&str, usize)>) -> Fault {
    let severity = if stall.wants_attention() {
        Severity::Warning
    } else {
        Severity::Advisory
    };
    // Where several items report one cause, the cause is what is wrong and the
    // items are how it showed. Naming an item there would send the operator to a
    // download to fix something that is not about that download.
    let summary = shared.map_or_else(
        || match item.cause.as_deref() {
            // What the service actually said beats what is usually the matter.
            Some(cause) => format!("{} — {}: {cause}", item.name, stall.word()),
            // Nothing said why, so say what usually is: the operator learns where to
            // look before they have opened anything, and a stall with no explanation
            // at all is the one they are least equipped to start on.
            None => format!(
                "{} — {}, typically {}",
                item.name,
                stall.word(),
                stall.typically()
            ),
        },
        |(cause, items)| format!("{items} downloads are blocked: {cause}"),
    );
    let mut fault = Fault::new(
        &format!("{CHECK}.{}", kind_of(stall)),
        severity,
        &summary,
        stall.means(),
        &stall.first_remedy(),
    );
    for remedy in stall.remedies().into_iter().skip(1) {
        fault = fault.or_else(&remedy);
    }
    fault
}

/// The event kind a category is, so four items stuck the same way are one alert.
const fn kind_of(stall: Stall) -> &'static str {
    match stall {
        Stall::RedownloadLoop => "redownload-loop",
        Stall::RepeatedImportFailure => "import-failing",
        Stall::CompletedNotImported => "not-imported",
        Stall::Orphaned => "orphaned",
        Stall::StalledDownload => "stalled",
        Stall::WaitingIndefinitely => "waiting",
        Stall::Slow => "slow",
    }
}

#[cfg(test)]
mod tests;
