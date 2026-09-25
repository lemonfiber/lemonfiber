use super::{window_start, Duration, SystemTime, DAILY_WINDOW, EPOCH, UNIX_EPOCH};

/// The moment a fixed clock reads, so a window taken back from it is the same every
/// run: noon on a day the arithmetic can be checked by hand.
fn noon() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_786_968_000)
}

#[test]
fn a_window_is_taken_back_from_the_moment_it_is_asked_at() {
    assert_eq!(window_start(noon(), DAILY_WINDOW), "2026-08-16T12:00:00");
}

/// A clock with less than a window behind it has nothing to take one back from, and
/// asking from the beginning reads every row there is rather than none.
#[test]
fn a_clock_with_no_room_behind_it_asks_from_the_beginning() {
    assert_eq!(window_start(UNIX_EPOCH, DAILY_WINDOW), EPOCH);
}

/// A clock so far out that no calendar holds it is a machine whose time is wrong,
/// which is worth reading past rather than refusing over.
#[test]
fn a_clock_beyond_any_calendar_asks_from_the_beginning_too() {
    let absurd = UNIX_EPOCH + Duration::from_secs(3_000_000_000_000);
    assert_eq!(window_start(absurd, DAILY_WINDOW), EPOCH);
}
