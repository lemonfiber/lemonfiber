use super::{read, written, Duration, SystemTime, UNIX_EPOCH};

/// A moment with every field in it, so a formatter that dropped one would show.
const STAMPED: &str = "2026-08-17T21:04:09";

fn at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
}

#[test]
fn an_instant_survives_being_written_and_read_back() {
    let moment = read(STAMPED).unwrap_or(UNIX_EPOCH);
    assert_eq!(written(moment).as_deref(), Some(STAMPED));
}

#[test]
fn a_stamp_is_read_as_the_second_it_names() {
    assert_eq!(read("1970-01-01T00:00:00"), Some(at(0)));
    assert_eq!(read("1970-01-02T00:00:01"), Some(at(86_401)));
    assert_eq!(read("2026-08-17T00:00:00"), Some(at(1_786_924_800)));
}

/// What a service appends to the second is not more certainty about which second it
/// was, so it is dropped rather than refused.
#[test]
fn fractions_and_a_zone_marker_are_dropped_rather_than_refused() {
    let plain = read("2026-08-17T21:04:09");
    assert_eq!(read("2026-08-17T21:04:09Z"), plain);
    assert_eq!(read("2026-08-17T21:04:09.482913Z"), plain);
}

/// A stamp naming its own offset is in a frame this does not read, and reading it as
/// though it were UTC would put a window hours from the rows it is meant to cover.
#[test]
fn a_stamp_in_another_frame_is_not_read_at_all() {
    assert_eq!(read("2026-08-17T21:04:09+02:00"), None);
    assert_eq!(read("2026-08-17T21:04:09.48+02:00"), None);
}

#[test]
fn what_cannot_be_a_time_is_not_one() {
    assert_eq!(read(""), None);
    assert_eq!(read("2026-08-17"), None);
    assert_eq!(read("not-a-day T21:04:09"), None);
    assert_eq!(read("2026-08-17T21:04"), None);
    assert_eq!(read("2026-08-17T21:04:09:11"), None);
    assert_eq!(read("2026-08-17T24:00:00"), None);
    assert_eq!(read("2026-08-17T21:60:00"), None);
    assert_eq!(read("2026-08-17T21:04:60"), None);
    assert_eq!(read("2026-08-17Txx:04:09"), None);
}

/// Nothing in the stack records anything before the epoch, and an instant that
/// claims to is not one this can write — or read back, since the count of seconds
/// every service keeps its history in starts there.
#[test]
fn an_instant_before_the_epoch_is_neither_written_nor_read() {
    assert_eq!(written(UNIX_EPOCH - Duration::from_secs(1)), None);
    assert_eq!(read("1969-12-31T23:59:59"), None);
}
