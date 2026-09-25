use super::{Minute, Quiet};
use std::time::{Duration, SystemTime};

/// An instant, as seconds since the epoch.
fn at(seconds: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
}

/// 2026-01-15, at the given hour UTC — deep winter, so a northern zone is on
/// standard time and one hour ahead.
fn winter(hour: u64) -> SystemTime {
    at(1_768_435_200 + hour * 3_600)
}

#[test]
fn a_window_reads_as_two_times_of_day() {
    let read = Quiet::parse("22:00-07:00", "UTC");
    assert_eq!(
        read,
        Some(Quiet {
            from: Minute(22 * 60),
            to: Minute(7 * 60),
            zone: "UTC".to_owned()
        })
    );
}

#[test]
fn a_window_may_be_written_with_spaces_around_it() {
    assert!(Quiet::parse("  09:30 - 17:45  ", "UTC").is_some());
}

/// Guessing here means silence in the wrong hours, so anything unreadable is no
/// window rather than an approximation of one.
#[test]
fn anything_that_does_not_read_as_a_window_is_no_window() {
    for said in [
        "",
        "22:00",
        "22-07",
        "25:00-07:00",
        "22:60-07:00",
        "ten-eleven",
    ] {
        assert!(Quiet::parse(said, "UTC").is_none(), "{said}");
    }
}

/// A window with no width would read as active while muting nothing.
#[test]
fn a_window_with_no_width_is_no_window() {
    assert!(Quiet::parse("22:00-22:00", "UTC").is_none());
}

#[test]
fn a_daytime_window_holds_between_its_ends() {
    let window = Quiet::parse("09:00-17:00", "UTC");
    assert!(
        window.as_ref().is_some_and(|held| held.holds(winter(9))),
        "at nine"
    );
    assert!(
        window.as_ref().is_some_and(|held| held.holds(winter(16))),
        "at four"
    );
    assert!(
        window.as_ref().is_some_and(|held| !held.holds(winter(8))),
        "before it"
    );
    assert!(
        window.as_ref().is_some_and(|held| !held.holds(winter(17))),
        "at its end"
    );
}

/// The ordinary case, and the one a naive comparison gets backwards.
#[test]
fn a_window_across_midnight_is_one_window_and_not_seventeen_hours() {
    let window = Quiet::parse("22:00-07:00", "UTC");
    assert!(
        window.as_ref().is_some_and(|held| held.holds(winter(23))),
        "late evening"
    );
    assert!(
        window.as_ref().is_some_and(|held| held.holds(winter(3))),
        "small hours"
    );
    assert!(
        window.as_ref().is_some_and(|held| !held.holds(winter(12))),
        "the middle of the day"
    );
    assert!(
        window.as_ref().is_some_and(|held| !held.holds(winter(7))),
        "at its end"
    );
}

/// The whole reason the zone is read rather than assumed: the same instant is a
/// different hour in two places, and the window belongs to the household.
#[test]
fn the_same_instant_falls_differently_in_two_zones() {
    let window = Quiet::parse("22:00-07:00", "UTC");
    // 21:30 UTC is 22:30 in Amsterdam in winter — inside there, outside here.
    let evening = winter(21) + std::time::Duration::from_secs(1_800);
    let theirs = Quiet::parse("22:00-07:00", "Europe/Amsterdam");
    assert!(
        theirs.as_ref().is_some_and(|held| held.holds(evening)),
        "in Amsterdam"
    );
    assert!(
        window.as_ref().is_some_and(|held| !held.holds(evening)),
        "in UTC"
    );
}

/// A typo in `TZ` must not mute anything: being told at the wrong hour beats not
/// being told.
#[test]
fn a_zone_the_database_does_not_know_holds_nothing() {
    let window = Quiet::parse("00:00-23:59", "Nowhere/Atlantis");
    assert!(window.as_ref().is_some_and(|held| !held.holds(winter(3))));
}
