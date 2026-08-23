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
        let held = spoken(self.held_for);
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

/// A duration in the words a person uses for it.
///
/// Hours stay hours until two days, because "36 hours" tells an operator more
/// about a stall than "2 days" does; past that the precision stops helping.
fn spoken(seconds: u64) -> String {
    let (count, unit) = match seconds {
        0..=5399 => (seconds.max(60) / 60, "minute"),
        5400..=172_799 => ((seconds + 1_800) / 3_600, "hour"),
        _ => ((seconds + 43_200) / 86_400, "day"),
    };
    // Saturating rather than `as`: a count large enough to truncate is a clock
    // that has gone wrong, and reporting "1 minute" for it would be worse than
    // reporting an implausibly large number honestly.
    let plural = usize::try_from(count).unwrap_or(usize::MAX);
    format!("{count} {unit}{}", crate::plural::s(plural))
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
mod tests {
    use std::time::Duration;

    use super::{assess, spoken, Stuck, LOOPING, REPEATED};
    use crate::queue::{Fetching, Importing, Item, Stall, Thresholds};

    /// Long enough that every threshold here has been passed.
    const AGES: Duration = Duration::from_secs(30 * 24 * 60 * 60);

    /// An item part-way through a download, an *arr waiting for it.
    fn downloading(name: &str, progress: u8, moving: bool) -> Item {
        Item {
            fetching: Some(Fetching { progress, moving }),
            importing: Some(Importing {
                failures: 0,
                imported: false,
            }),
            held_for: AGES,
            ..Item::named(name)
        }
    }

    /// What `assess` made of one item.
    fn verdict(item: Item) -> Option<Stall> {
        assess(&[item], Thresholds::conservative())
            .first()
            .map(|stuck| stuck.stall)
    }

    #[test]
    fn a_download_that_has_not_moved_in_hours_is_stalled() {
        assert_eq!(
            verdict(downloading("Some.Release", 42, false)),
            Some(Stall::StalledDownload)
        );
    }

    #[test]
    fn a_download_that_is_moving_is_at_worst_slow() {
        // Something still arriving needs patience, not intervention.
        assert_eq!(
            verdict(downloading("Some.Release", 42, true)),
            Some(Stall::Slow)
        );
    }

    #[test]
    fn a_download_still_within_its_threshold_says_nothing_at_all() {
        // The defaults are meant to be wrong in the direction of silence.
        let fresh = Item {
            held_for: Duration::from_secs(60),
            ..downloading("Some.Release", 42, false)
        };
        assert_eq!(verdict(fresh), None);
    }

    #[test]
    fn something_finished_and_never_imported_is_named_as_that() {
        // The failure nobody owns.
        assert_eq!(
            verdict(downloading("Some.Release", 100, false)),
            Some(Stall::CompletedNotImported)
        );
    }

    #[test]
    fn a_finished_download_nothing_is_waiting_for_is_orphaned_instead() {
        let orphan = Item {
            importing: None,
            ..downloading("Some.Release", 100, false)
        };
        assert_eq!(verdict(orphan), Some(Stall::Orphaned));
    }

    #[test]
    fn a_file_kept_for_seeding_after_import_is_not_reported_at_all() {
        // 100% and sitting in the client for ever is what seeding looks like, and
        // reporting it would flag every healthy torrent on the machine.
        let seeding = Item {
            importing: Some(Importing {
                failures: 0,
                imported: true,
            }),
            ..downloading("Some.Release", 100, false)
        };
        assert_eq!(verdict(seeding), None);
    }

    #[test]
    fn the_same_item_fetched_again_and_again_is_a_loop_before_it_is_anything_else() {
        // Whatever else is true of it right now, the loop is what is spending the
        // allowance — and reporting "not moving" would send the operator to the
        // torrent when the problem is an import failing silently underneath.
        let looping = Item {
            grabs: LOOPING,
            ..downloading("Some.Release", 42, false)
        };
        assert_eq!(verdict(looping), Some(Stall::RedownloadLoop));
    }

    #[test]
    fn a_second_fetch_is_a_retry_rather_than_a_loop() {
        // A system retrying once is a system working.
        let retried = Item {
            grabs: LOOPING - 1,
            ..downloading("Some.Release", 42, false)
        };
        assert_eq!(verdict(retried), Some(Stall::StalledDownload));
    }

    #[test]
    fn an_import_that_keeps_failing_is_structural_and_said_so() {
        let failing = Item {
            importing: Some(Importing {
                failures: REPEATED,
                imported: false,
            }),
            ..downloading("Some.Release", 100, false)
        };
        assert_eq!(verdict(failing), Some(Stall::RepeatedImportFailure));
    }

    #[test]
    fn something_monitored_and_never_grabbed_is_waiting() {
        let wanted = Item {
            fetching: None,
            ..downloading("Some.Film", 0, false)
        };
        assert_eq!(verdict(wanted), Some(Stall::WaitingIndefinitely));
    }

    #[test]
    fn an_item_the_operator_set_aside_is_never_reported() {
        // Already judged. A check that keeps raising something dismissed is one
        // that gets dismissed itself.
        let dismissed = Item {
            unmanaged: true,
            ..downloading("Some.Release", 42, false)
        };
        assert!(assess(&[dismissed], Thresholds::conservative()).is_empty());
    }

    #[test]
    fn the_worst_thing_is_at_the_top_and_two_runs_read_alike() {
        let items = vec![
            downloading("b.slow", 42, true),
            Item {
                grabs: LOOPING,
                ..downloading("a.loop", 42, false)
            },
            downloading("c.stalled", 42, false),
        ];
        let order: Vec<Stall> = assess(&items, Thresholds::conservative())
            .iter()
            .map(|stuck| stuck.stall)
            .collect();
        assert_eq!(
            order,
            vec![Stall::RedownloadLoop, Stall::StalledDownload, Stall::Slow]
        );
    }

    #[test]
    fn two_things_equally_wrong_are_ordered_by_how_long_then_by_name() {
        let items = vec![
            Item {
                held_for: AGES,
                ..downloading("b", 42, false)
            },
            Item {
                held_for: AGES + Duration::from_secs(1),
                ..downloading("c", 42, false)
            },
            Item {
                held_for: AGES,
                ..downloading("a", 42, false)
            },
        ];
        let order: Vec<String> = assess(&items, Thresholds::conservative())
            .iter()
            .map(|stuck| stuck.name.clone())
            .collect();
        assert_eq!(order, vec!["c".to_owned(), "a".to_owned(), "b".to_owned()]);
    }

    #[test]
    fn a_stuck_item_says_which_one_what_is_wrong_and_for_how_long() {
        // "3 items stuck" is a status line. This is the sentence instead of it.
        let stuck = Stuck {
            name: "Some.Release".to_owned(),
            stall: Stall::StalledDownload,
            held_for: 7 * 60 * 60,
            blocking: None,
            items: 1,
        };
        assert_eq!(stuck.said(), "Some.Release — not moving for 7 hours");
    }

    #[test]
    fn an_item_whose_cause_the_service_named_says_it() {
        // The difference between "stuck" and something an operator can fix, in the
        // words of the thing that refused.
        let stuck = Stuck {
            name: "Some.Release".to_owned(),
            stall: Stall::StalledDownload,
            held_for: 7 * 60 * 60,
            blocking: Some("No space left on device".to_owned()),
            items: 1,
        };
        assert_eq!(
            stuck.said(),
            "Some.Release — not moving for 7 hours: No space left on device"
        );
    }

    #[test]
    fn a_cause_stopping_several_leads_with_the_cause_rather_than_an_item() {
        // Twenty downloads stopped by a full disk are one thing to fix. Naming the
        // items would bury the sentence that matters, and naming one of them would
        // send the operator to a download to fix something that is not about it.
        let stuck = Stuck {
            name: "No space left on device".to_owned(),
            stall: Stall::StalledDownload,
            held_for: 7 * 60 * 60,
            blocking: Some("No space left on device".to_owned()),
            items: 20,
        };
        assert_eq!(
            stuck.said(),
            "20 items — not moving for 7 hours: No space left on device"
        );
    }

    #[test]
    fn a_duration_reads_in_the_units_a_person_would_use() {
        assert_eq!(spoken(0), "1 minute", "never nothing at all");
        assert_eq!(spoken(60), "1 minute");
        assert_eq!(spoken(30 * 60), "30 minutes");
        assert_eq!(spoken(2 * 60 * 60), "2 hours");
        // Hours stay hours until two days, because "36 hours" tells an operator
        // more about a stall than "2 days" does.
        assert_eq!(spoken(24 * 60 * 60), "24 hours");
        assert_eq!(spoken(47 * 60 * 60), "47 hours");
        assert_eq!(spoken(48 * 60 * 60), "2 days");
        assert_eq!(spoken(8 * 24 * 60 * 60), "8 days");
    }

    #[test]
    fn what_kind_of_stall_it_is_can_be_read_without_knowing_how_long() {
        // The two have different sources: the category is readable from the
        // services right now, the age is not — neither side reports it.
        assert_eq!(
            super::category(&downloading("Some.Release", 42, false)),
            Some(Stall::StalledDownload)
        );
        let fresh = Item {
            held_for: Duration::ZERO,
            ..downloading("Some.Release", 42, false)
        };
        assert_eq!(
            super::category(&fresh),
            Some(Stall::StalledDownload),
            "the same category however new it is"
        );
    }

    #[test]
    fn something_structural_is_said_as_soon_as_it_is_seen() {
        // A loop and a repeated import failure will not resolve themselves, and
        // waiting only spends more of the allowance.
        let thresholds = Thresholds::conservative();
        assert_eq!(thresholds.for_stall(Stall::RedownloadLoop), Duration::ZERO);
        assert_eq!(
            thresholds.for_stall(Stall::RepeatedImportFailure),
            Duration::ZERO
        );
        let looping = Item {
            grabs: LOOPING,
            held_for: Duration::ZERO,
            ..downloading("Some.Release", 42, false)
        };
        assert_eq!(verdict(looping), Some(Stall::RedownloadLoop));
    }

    #[test]
    fn every_category_has_a_threshold_and_only_the_structural_ones_are_immediate() {
        let thresholds = Thresholds::conservative();
        let immediate: Vec<Stall> = Stall::ALL
            .into_iter()
            .filter(|stall| thresholds.for_stall(*stall).is_zero())
            .collect();
        assert_eq!(
            immediate,
            vec![Stall::RedownloadLoop, Stall::RepeatedImportFailure]
        );
    }

    #[test]
    fn items_stopped_by_one_cause_are_assessed_as_that_cause() {
        // The pure path, without a store: a full disk stops every download on the
        // machine, and reporting it once per item buries the one thing to fix.
        let blocked = |name: &str| Item {
            cause: Some("No space left on device".to_owned()),
            ..downloading(name, 40, false)
        };
        let assessed = assess(
            &[blocked("First"), blocked("Second"), blocked("Third")],
            Thresholds::conservative(),
        );
        assert_eq!(assessed.len(), 1, "{assessed:?}");
        assert_eq!(
            assessed
                .first()
                .map(|stuck| (stuck.name.clone(), stuck.items)),
            Some(("No space left on device".to_owned(), 3))
        );
    }

    #[test]
    fn one_item_with_a_cause_keeps_its_own_name() {
        let alone = Item {
            cause: Some("Permission denied".to_owned()),
            ..downloading("Only.Release", 40, false)
        };
        let assessed = assess(&[alone], Thresholds::conservative());
        assert_eq!(
            assessed.first().map(|stuck| stuck.name.clone()),
            Some("Only.Release".to_owned())
        );
    }
}
