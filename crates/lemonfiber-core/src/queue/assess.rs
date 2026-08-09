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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stuck {
    /// Which item.
    pub name: String,
    /// What is wrong with it.
    pub stall: Stall,
    /// How long it has been that way, in seconds — what turns "stuck" into a
    /// sentence an operator can weigh.
    pub held_for: u64,
}

impl Stuck {
    /// The line an operator reads.
    #[must_use]
    pub fn said(&self) -> String {
        format!(
            "{} — {} for {}",
            self.name,
            self.stall.word(),
            spoken(self.held_for)
        )
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
            stall_of(item, thresholds).map(|stall| Stuck {
                name: item.name.clone(),
                stall,
                held_for: item.held_for.as_secs(),
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
    stuck
}

/// What is wrong with one item, or nothing where it is simply working.
///
/// Checked worst-first because an item can be several of these at once, and the
/// one that sends the operator to the right place is the worst true one.
fn stall_of(item: &Item, thresholds: Thresholds) -> Option<Stall> {
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
        return past(thresholds.not_imported, item).then_some(if item.is_orphaned() {
            Stall::Orphaned
        } else {
            Stall::CompletedNotImported
        });
    }
    if item.is_waiting() {
        return past(thresholds.waiting, item).then_some(Stall::WaitingIndefinitely);
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
    if !moving && past(thresholds.stalled, item) {
        return Some(Stall::StalledDownload);
    }
    (moving && past(thresholds.slow, item)).then_some(Stall::Slow)
}

/// Whether an item has been in its state longer than a threshold allows.
fn past(threshold: std::time::Duration, item: &Item) -> bool {
    !Thresholds::conservative().within(threshold, item.held_for)
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
        };
        assert_eq!(stuck.said(), "Some.Release — not moving for 7 hours");
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
}
