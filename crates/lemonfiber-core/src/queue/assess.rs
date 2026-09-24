//! Deciding what, if anything, is wrong with the queue.
//!
//! One pass over items that already hold both sides of the story, so the rules
//! read as the operator would state them rather than as two services' APIs
//! happen to be shaped.
//!
//! Order matters here and is the whole design. An item can satisfy several
//! categories at once — something fetched four times is also, right now, an
//! incomplete download — and reporting it as "not moving" would send the operator
//! to the torrent when the problem is an import failing silently underneath.
//! So the worst true thing wins, and the categories are checked in that order.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{Item, Stall, Thresholds};

/// How many times an item may be fetched before the fetching is the fault.
///
/// Twice is a retry, which is a system working. By the third the retry is the
/// problem: something is failing quietly and being asked again, and it will go on
/// doing that until somebody stops it.
pub const LOOPING: u32 = 3;

/// How many import failures make it structural rather than unlucky.
pub const REPEATED: u32 = 2;

/// One thing that is wrong, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Stuck {
    /// Which item — or, where several share one cause, that cause.
    pub name: String,
    /// What is wrong with it.
    pub stall: Stall,
    /// How long it has been that way, in seconds — what turns "stuck" into a
    /// sentence an operator can weigh.
    pub held_for: u64,
    /// What the service said was blocking it, in its own words, where it said
    /// anything. A permission denial from an import log is worth more than any
    /// interpretation of it, and it is the difference between "stuck" and
    /// something an operator can fix.
    pub blocking: Option<String>,
    /// How many items this stands for. One in the ordinary case; more where they
    /// share a cause and the cause is what is wrong — twenty downloads stopped by
    /// a full disk are one thing to fix, and twenty alerts about it are how an
    /// operator learns to mute the queue check.
    pub items: usize,
}

impl Stuck {
    /// The line an operator reads.
    #[must_use]
    pub fn said(&self) -> String {
        let held = crate::spoken::duration(self.held_for);
        let cause = self
            .blocking
            .as_deref()
            .map(|cause| format!(": {cause}"))
            .unwrap_or_default();
        if self.items > 1 {
            // The cause leads, because it is the thing to fix. Naming twenty items
            // would bury the one sentence that matters.
            return format!(
                "{} items — {} for {held}{cause}",
                self.items,
                self.stall.word()
            );
        }
        format!("{} — {} for {held}{cause}", self.name, self.stall.word())
    }
}

/// Everything wrong with the queue, worst first.
///
/// Items the operator marked unmanaged are absent entirely: they have already been
/// judged, and a check that keeps raising something already dismissed is one that
/// gets dismissed itself.
#[must_use]
pub fn assess(items: &[Item], thresholds: Thresholds) -> Vec<Stuck> {
    let mut stuck: Vec<Stuck> = items
        .iter()
        .filter(|item| !item.unmanaged)
        .filter_map(|item| {
            let stall = category(item)?;
            // Long enough to be worth saying, by whatever age the caller knows.
            (!thresholds.within(thresholds.for_stall(stall), item.held_for)).then(|| Stuck {
                name: item.name.clone(),
                stall,
                held_for: item.held_for.as_secs(),
                blocking: item.cause.clone(),
                items: 1,
            })
        })
        .collect();
    // Worst first, then longest, then by name — so two runs over one stack read
    // alike and the thing to act on is at the top.
    stuck.sort_by(|left, right| {
        left.stall
            .cmp(&right.stall)
            .then_with(|| right.held_for.cmp(&left.held_for))
            .then_with(|| left.name.cmp(&right.name))
    });
    attributed(stuck)
}

/// Collapse the items sharing one blocking cause into that cause.
///
/// A full disk stops every download on the machine. Reporting it twenty times —
/// once per item, each with the same sentence — buries the one thing to fix and
/// teaches an operator to mute the check that would have told them. The cause is
/// what is wrong; the items are how it showed.
///
/// Only where more than one item reports it. A single item blocked by something is
/// that item's problem, and naming the cause instead of the item would lose which
/// download to look at.
#[must_use]
pub fn attributed(stuck: Vec<Stuck>) -> Vec<Stuck> {
    let mut sharing: BTreeMap<String, usize> = BTreeMap::new();
    for entry in &stuck {
        if let Some(cause) = entry.blocking.clone() {
            *sharing.entry(cause).or_default() += 1;
        }
    }
    let mut attributed: Vec<Stuck> = Vec::new();
    let mut spoken_for: BTreeSet<String> = BTreeSet::new();
    for entry in stuck {
        let shared = entry
            .blocking
            .as_deref()
            .filter(|cause| sharing.get(*cause).copied().unwrap_or(0) > 1)
            .map(str::to_owned);
        let Some(cause) = shared else {
            attributed.push(entry);
            continue;
        };
        // The first one carries the group: the list is already worst-first and
        // longest-first, so the entry that leads is the worst and oldest of them.
        if spoken_for.insert(cause.clone()) {
            attributed.push(Stuck {
                name: cause.clone(),
                items: sharing.get(cause.as_str()).copied().unwrap_or(1),
                ..entry
            });
        }
    }
    attributed
}

/// Which category an item falls in, before any question of how long.
///
/// Separated from the threshold because the two have different sources. What kind
/// of stall this is can be read from the services right now; *how long it has been
/// that way* cannot — neither side reports it, and time since the item was added
/// is a different measurement that would call a download added three days ago and
/// stalled ten minutes ago "stalled for three days".
///
/// So a caller with a memory — the condition store, which stamps when a fault was
/// first seen — applies [`Thresholds::for_stall`] to the age it knows. A caller
/// without one passes the age it has and uses [`assess`].
///
/// Checked worst-first because an item can be several of these at once, and the
/// one that sends the operator to the right place is the worst true one.
#[must_use]
pub fn category(item: &Item) -> Option<Stall> {
    // Being fetched over and over outranks everything: whatever else is true of
    // this item right now, the loop is what is spending the allowance.
    if item.grabs >= LOOPING {
        return Some(Stall::RedownloadLoop);
    }
    if item
        .importing
        .is_some_and(|importing| importing.failures >= REPEATED)
    {
        return Some(Stall::RepeatedImportFailure);
    }
    if item.is_completed_not_imported() {
        return Some(if item.is_orphaned() {
            Stall::Orphaned
        } else {
            Stall::CompletedNotImported
        });
    }
    if item.is_waiting() {
        return Some(Stall::WaitingIndefinitely);
    }
    // A finished download is not a stalled one. Past this point the transfer is
    // still running, so a complete one has already been accounted for above —
    // reaching the stall rules with it would report every seeding torrent on the
    // machine as stuck, which is the fastest way to have the whole check muted.
    if item.fetching.is_some_and(super::Fetching::is_complete) {
        return None;
    }
    // Still fetching. Not moving at all is a stall; moving slowly is a note.
    let moving = item.fetching.is_some_and(|fetching| fetching.moving);
    Some(if moving {
        Stall::Slow
    } else {
        Stall::StalledDownload
    })
}

#[cfg(test)]
mod tests;
