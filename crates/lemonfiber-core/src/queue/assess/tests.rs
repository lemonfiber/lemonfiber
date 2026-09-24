use std::time::Duration;

use super::{assess, Stuck, LOOPING, REPEATED};
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
