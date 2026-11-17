use lemonfiber_manifest::schedule::Cron;

use super::{fired, minute_of, zone, Fired};
use crate::instant::{Instant, DAY, HOUR, MINUTE};

/// 2026-01-15 00:00 UTC, a Thursday in winter.
const THURSDAY: u64 = 1_768_435_200;

fn cron(text: &str) -> Cron {
    Cron::parse(text).unwrap_or_else(|| unreachable!("a schedule"))
}

fn at(seconds: u64) -> Instant {
    Instant::from_seconds(THURSDAY + seconds)
}

#[test]
fn a_minute_starts_on_its_first_second() {
    assert_eq!(minute_of(at(HOUR + 59)), at(HOUR));
    assert_eq!(minute_of(at(HOUR)), at(HOUR));
}

#[test]
fn the_minutes_after_the_last_considered_up_to_now_are_read() {
    let utc = zone(Some("UTC"));
    let hourly = cron("@hourly");
    assert_eq!(
        fired(&hourly, &utc, at(0), at(HOUR - MINUTE)),
        Fired::default()
    );
    assert_eq!(
        fired(&hourly, &utc, at(0), at(HOUR + 30)),
        Fired {
            count: 1,
            last: Some(at(HOUR)),
        }
    );
    assert_eq!(
        fired(&hourly, &utc, at(HOUR), at(3 * HOUR)),
        Fired {
            count: 2,
            last: Some(at(3 * HOUR)),
        }
    );
}

#[test]
fn the_times_are_read_on_the_zones_wall_clock() {
    let amsterdam = zone(Some("Europe/Amsterdam"));
    let four = cron("0 4 * * *");
    let fired = fired(&four, &amsterdam, at(0), at(DAY));
    assert_eq!(fired.last, Some(at(3 * HOUR)));
    assert_eq!(fired.count, 1);
}

#[test]
fn a_gap_longer_than_a_year_is_read_from_a_year_back() {
    let utc = zone(Some("UTC"));
    let fired = fired(&cron("@daily"), &utc, Instant::EPOCH, at(0));
    assert_eq!(fired.count, 367);
    assert_eq!(fired.last, Some(at(0)));
}

#[test]
fn a_zone_nobody_names_or_nobody_knows_is_the_hosts() {
    let host = jiff::tz::TimeZone::system();
    assert_eq!(zone(None), host);
    assert_eq!(zone(Some(" ")), host);
    assert_eq!(zone(Some("Nowhere/Atall")), host);
}
