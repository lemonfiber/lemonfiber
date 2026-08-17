//! A calendar date, and the little arithmetic checking one needs.
//!
//! Separate from validation because it is calendar code, not a manifest rule:
//! reading a `YYYY-MM-DD` field and turning a moment into the day it falls on.

/// A calendar date, for checking one recorded in a manifest.
///
/// Ordering is derived, which is chronological because the fields are declared
/// most-significant first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    /// Four-digit year.
    pub year: u16,
    /// Month, 1 to 12.
    pub month: u8,
    /// Day, 1 to 31.
    pub day: u8,
}

impl Date {
    /// Read a `YYYY-MM-DD` date, rejecting anything else.
    ///
    /// Deliberately strict about shape and about whether the parts are possible.
    /// It does not know how long February is: a date being *real* matters less
    /// than it being unambiguous, and a manifest claiming the 30th of February
    /// has a bigger problem than this check.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let mut parts = text.split('-');
        let year = parts.next()?;
        let month = parts.next()?;
        let day = parts.next()?;
        if parts.next().is_some() {
            return None;
        }

        // Each field is a fixed width of digits and nothing else. A bare length
        // check let `2026-006-1` and a `+`-signed part through, because an integer
        // parse accepts either; requiring exact digit widths is what actually
        // pins the one unambiguous shape.
        let shaped = year.len() == 4
            && month.len() == 2
            && day.len() == 2
            && [year, month, day]
                .iter()
                .all(|part| part.bytes().all(|byte| byte.is_ascii_digit()));
        if !shaped {
            return None;
        }

        let year: u16 = year.parse().ok()?;
        let month: u8 = month.parse().ok()?;
        let day: u8 = day.parse().ok()?;
        ((1..=12).contains(&month) && (1..=31).contains(&day)).then_some(Self { year, month, day })
    }

    /// The date at a moment, given as seconds since the Unix epoch, in UTC.
    ///
    /// Days-to-calendar conversion rather than a date library: this is the only
    /// calendar arithmetic in the codebase, and a dependency that parses time
    /// zones and formats a dozen ways would be carried for one function.
    ///
    /// The algorithm is Howard Hinnant's `civil_from_days`, which shifts the
    /// year to start in March so leap days fall at the end of it and no month
    /// needs a special case.
    #[must_use]
    pub fn from_unix_seconds(seconds: i64) -> Option<Self> {
        let days = seconds.div_euclid(86_400);
        let shifted = days + 719_468;
        let era = shifted.div_euclid(146_097);
        let day_of_era = shifted - era * 146_097;
        let year_of_era =
            (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let mut year = year_of_era + era * 400;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let shifted_month = (5 * day_of_year + 2) / 153;
        let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
        let month = if shifted_month < 10 {
            shifted_month + 3
        } else {
            shifted_month - 9
        };
        if month <= 2 {
            year += 1;
        }

        Some(Self {
            year: u16::try_from(year).ok()?,
            month: u8::try_from(month).ok()?,
            day: u8::try_from(day).ok()?,
        })
    }

    /// How many days from this date to `other` — negative where `other` is earlier.
    ///
    /// Counting days between calendar dates is the one operation subtraction of the
    /// fields cannot do: months are different lengths and Februaries are two lengths,
    /// so both dates are converted to a day count first and the difference taken there.
    #[must_use]
    pub fn days_until(self, other: Self) -> i64 {
        other.epoch_day() - self.epoch_day()
    }

    /// The date as days since the Unix epoch — the inverse of the conversion above,
    /// and Hinnant's `days_from_civil` for the same reason: shifting the year to start
    /// in March puts the leap day at the end of it, where it needs no special case.
    fn epoch_day(self) -> i64 {
        let year = i64::from(self.year) - i64::from(self.month <= 2);
        let era = year.div_euclid(400);
        let year_of_era = year - era * 400;
        let shifted_month = i64::from(self.month) + if self.month > 2 { -3 } else { 9 };
        let day_of_year = (153 * shifted_month + 2) / 5 + i64::from(self.day) - 1;
        let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
        era * 146_097 + day_of_era - 719_468
    }
}

#[cfg(test)]
mod tests {
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
}
