use super::{Level, CRITICAL_FLOOR, EXHAUSTED_FLOOR};
use crate::doctor::storage::LOW_SPACE_FLOOR;

/// A volume of a terabyte, which is large enough that the relative step never
/// fires by accident in the cases about the absolute ones.
const LARGE: u64 = 1024 * 1024 * 1024 * 1024;

#[test]
fn a_volume_nobody_could_read_is_unknown_rather_than_full() {
    assert_eq!(Level::reached(None, Some(LARGE), 0), Level::Unknown);
    assert_eq!(Level::reached(Some(500), None, 0), Level::Unknown);
    assert_eq!(
        Level::reached(Some(500), Some(0), 0),
        Level::Unknown,
        "a volume reporting no size at all was not measured"
    );
    assert!(!Level::Unknown.worth_saying());
    assert!(!Level::Unknown.halts());
}

#[test]
fn a_volume_with_room_to_spare_is_ample() {
    let level = Level::reached(Some(LARGE / 2), Some(LARGE), 0);
    assert_eq!(level, Level::Ample);
    assert!(
        !level.worth_saying(),
        "nothing to say about a healthy volume"
    );
}

#[test]
fn exhaustion_is_projected_from_what_is_already_committed() {
    // The requirement this whole module exists for: forty gigabytes free and a
    // queue that will take thirty-five of them is a volume about to fill, and
    // the free figure on its own says it is fine.
    let volume = 100 * 1024 * 1024 * 1024;
    let free = 40 * 1024 * 1024 * 1024;
    let committed = 35 * 1024 * 1024 * 1024;
    assert_eq!(
        Level::reached(Some(free), Some(volume), 0),
        Level::Ample,
        "nothing committed, so the same volume is comfortable"
    );
    assert_eq!(
        Level::reached(Some(free), Some(volume), committed),
        Level::Warning
    );
}

#[test]
fn the_steps_escalate_in_order_as_the_projection_falls() {
    let ample = Level::reached(Some(LARGE / 2), Some(LARGE), 0);
    let advisory = Level::reached(Some(LARGE / 100), Some(LARGE), 0);
    let warning = Level::reached(
        Some(LOW_SPACE_FLOOR + CRITICAL_FLOOR),
        Some(LARGE),
        CRITICAL_FLOOR + 1,
    );
    let critical = Level::reached(Some(CRITICAL_FLOOR + 1), Some(LARGE), 2);
    let exhausted = Level::reached(Some(EXHAUSTED_FLOOR - 1), Some(LARGE), 0);
    assert_eq!(
        [ample, advisory, warning, critical, exhausted],
        [
            Level::Ample,
            Level::Advisory,
            Level::Warning,
            Level::Critical,
            Level::Exhausted
        ]
    );
    assert!(ample < advisory && advisory < warning && warning < critical && critical < exhausted);
}

#[test]
fn a_volume_below_comfortable_is_advisory_on_its_own_size() {
    // A twentieth of a terabyte is fifty gigabytes, which passes every absolute
    // floor and is still a volume worth mentioning before it becomes urgent.
    let level = Level::reached(Some(LARGE / 20), Some(LARGE), 0);
    assert_eq!(level, Level::Advisory);
    assert!(level.worth_saying());
    assert!(!level.halts());
}

#[test]
fn only_exhaustion_halts_new_work() {
    for level in [
        Level::Unknown,
        Level::Ample,
        Level::Advisory,
        Level::Warning,
        Level::Critical,
    ] {
        assert!(!level.halts(), "{} does not halt", level.word());
    }
    assert!(Level::Exhausted.halts());
    let means = Level::Exhausted.means();
    assert!(
        means.contains("databases"),
        "the halt says what it is protecting: {means}"
    );
}

#[test]
fn exhaustion_is_read_from_what_is_free_rather_than_from_the_projection() {
    // A volume with room now and a queue that will consume all of it is going
    // to be full; halting a working stack on a prediction would stop work that
    // still fits.
    let free = 20 * 1024 * 1024 * 1024;
    assert_eq!(
        Level::reached(Some(free), Some(LARGE), free),
        Level::Critical,
        "projected to nothing, and still writable now"
    );
}

#[test]
fn a_stack_is_as_well_off_as_its_worst_volume() {
    assert_eq!(
        Level::worst([Level::Ample, Level::Exhausted]),
        Level::Exhausted
    );
    assert_eq!(
        Level::worst([Level::Unknown, Level::Advisory]),
        Level::Advisory,
        "an unread volume never outranks a measured one"
    );
    assert_eq!(Level::worst([]), Level::Unknown);
}

#[test]
fn every_level_reads_back_as_a_word_and_says_what_it_means() {
    let levels = [
        Level::Unknown,
        Level::Ample,
        Level::Advisory,
        Level::Warning,
        Level::Critical,
        Level::Exhausted,
    ];
    assert_eq!(levels.len(), 6, "every state the report can be in");
    for level in levels {
        let word = level.word();
        let means = level.means();
        assert!(!word.is_empty());
        assert!(means.len() > 20, "{word} says what it means: {means}");
    }
}
