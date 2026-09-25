use super::Date;

#[test]
fn a_date_is_read_only_in_the_one_shape_that_is_unambiguous() {
    assert_eq!(
        Date::parse("2026-06-26"),
        Some(Date {
            year: 2026,
            month: 6,
            day: 26
        })
    );
    for bad in [
        "26-06-26",
        "2026-ab-26",
        "2026-06-cd",
        "xxxx-06-26",
        "2026-6-26",
        "2026/06/26",
        "2026-13-01",
        "2026-00-01",
        "2026-06-32",
        "2026-06-00",
        "2026-06",
        "2026-06-26-01",
        // Misaligned digit groups and signed parts reach length ten but are
        // not the one unambiguous shape.
        "2026-006-1",
        "200-006-01",
        "2026-+6-26",
        "+026-06-26",
        "2026-06-2 ",
        "not a date",
        "",
    ] {
        assert_eq!(Date::parse(bad), None, "{bad:?} should not parse");
    }
}

#[test]
fn a_moment_becomes_the_day_it_falls_on() {
    // Checked against known instants rather than against another
    // implementation of the same arithmetic.
    for (seconds, expected) in [
        (0_i64, "1970-01-01"),
        (86_399, "1970-01-01"),
        (86_400, "1970-01-02"),
        (951_782_400, "2000-02-29"),
        (1_609_459_199, "2020-12-31"),
        (1_774_396_800, "2026-03-25"),
        (-1, "1969-12-31"),
    ] {
        assert_eq!(
            Date::from_unix_seconds(seconds),
            Date::parse(expected),
            "{seconds} should be {expected}"
        );
    }
}

#[test]
fn a_moment_no_calendar_can_name_is_refused_rather_than_wrapped() {
    // Far enough ahead that the year does not fit. Wrapping it would put a
    // manifest's dates in the wrong century rather than saying so.
    assert_eq!(Date::from_unix_seconds(i64::MAX / 2), None);
}

#[test]
fn dates_order_chronologically() {
    assert!(Date::parse("2025-12-31") < Date::parse("2026-01-01"));
    assert!(Date::parse("2026-01-01") < Date::parse("2026-01-02"));
    assert!(Date::parse("2026-01-01") < Date::parse("2026-02-01"));
}

/// The cases field subtraction gets wrong: month ends, year ends, and the two
/// lengths of February — including the century rule that makes 1900 an ordinary
/// year and 2000 a leap one.
#[test]
fn days_between_dates_count_the_calendar_rather_than_the_fields() {
    for (from, to, days) in [
        (day(2026, 6, 26), day(2026, 6, 26), 0_i64),
        (day(2026, 6, 26), day(2026, 6, 27), 1),
        (day(2026, 1, 31), day(2026, 2, 1), 1),
        (day(2025, 12, 31), day(2026, 1, 1), 1),
        (day(2024, 2, 28), day(2024, 3, 1), 2),
        (day(2023, 2, 28), day(2023, 3, 1), 1),
        (day(1900, 2, 28), day(1900, 3, 1), 1),
        (day(2000, 2, 28), day(2000, 3, 1), 2),
        (day(2026, 1, 1), day(2027, 1, 1), 365),
        (day(2024, 1, 1), day(2025, 1, 1), 366),
    ] {
        assert_eq!(from.days_until(to), days, "{from:?} to {to:?}");
        assert_eq!(to.days_until(from), -days, "{to:?} back to {from:?}");
    }
}

/// The two conversions are inverses, so a date read from a moment counts the same
/// days back to the epoch that the moment did.
#[test]
fn the_day_count_agrees_with_the_moment_it_came_from() {
    for seconds in [0_i64, 86_400, 951_782_400, 1_774_396_800, -86_400] {
        assert_eq!(
            Date::from_unix_seconds(seconds).map(|date| day(1970, 1, 1).days_until(date)),
            Some(seconds.div_euclid(86_400)),
            "{seconds} seconds"
        );
    }
}

/// Built rather than parsed, because the parse is what the tests above are for
/// and a test helper that can fail is a second thing to reason about.
const fn day(year: u16, month: u8, day: u8) -> Date {
    Date { year, month, day }
}
