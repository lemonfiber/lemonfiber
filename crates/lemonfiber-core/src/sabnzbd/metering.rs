//! What `SABnzbd` has pulled in a calendar month.
//!
//! This client is the one of the two that can answer the question properly: it
//! keeps a figure per day, keyed by the day, so a month is a sum over the days that
//! fall in it rather than a running total that forgets itself on restart.
//!
//! The days are taken across every account rather than per account. A monthly cap
//! belongs to the line, and two blocks bought from two providers are two allowances
//! over one connection — summing them is the whole point.

use async_trait::async_trait;

use lemonfiber_manifest::Date;

use crate::ports::service::{Failure, Metering, Moved};

use super::accounts::{daily_across, StatsResponse};
use super::Sabnzbd;

#[async_trait]
impl Metering for Sabnzbd {
    async fn moved(&self, month: &str) -> Result<Moved, Failure> {
        let measured: StatsResponse = self
            .read("server_stats", "the account statistics could not be read")
            .await?;
        Ok(Moved {
            down: pulled_in(&daily_across(&measured), month),
            // Usenet is a download and nothing else, so there is no upload to
            // count rather than an upload nobody counted.
            up: 0,
            since_start: false,
        })
    }
}

/// What the days falling in `month` add up to.
///
/// Saturating, so no sum of a provider's own counters can wrap; a day the client
/// dated in a way nothing could place is already absent by the time this sees it.
fn pulled_in(daily: &[(Date, u64)], month: &str) -> u64 {
    daily
        .iter()
        .filter(|(day, _)| falls_in(*day, month))
        .map(|(_, bytes)| *bytes)
        .fold(0, u64::saturating_add)
}

/// Whether a day falls in a `YYYY-MM` month.
fn falls_in(day: Date, month: &str) -> bool {
    written(day) == month.trim()
}

/// A day as the month it falls in.
fn written(day: Date) -> String {
    format!("{:04}-{:02}", day.year, day.month)
}

#[cfg(test)]
mod tests {
    use lemonfiber_fixtures::http::Fake;
    use lemonfiber_manifest::Date;

    use super::{falls_in, pulled_in, written, Metering, Sabnzbd};

    /// Two accounts, each with days in and out of the month asked about.
    ///
    /// A monthly cap belongs to the line and not to any one provider on it, so the
    /// two are summed rather than reported apart.
    const TWO_ACCOUNTS: &str = r#"{"servers":{
        "one":{"total":9000,"daily":{"2026-08-31":1000,"2026-09-01":2000}},
        "two":{"total":5000,"daily":{"2026-09-02":3000,"not-a-day":9}}
    }}"#;

    #[tokio::test]
    async fn a_month_is_summed_across_every_account_on_the_one_line() {
        let moved = Sabnzbd::new(
            Fake::scripted(vec![(200, TWO_ACCOUNTS)]),
            "http://127.0.0.1:8080",
            "the-key",
        )
        .moved("2026-09")
        .await;
        assert!(
            moved.is_ok_and(|moved| moved.down == 5_000 && moved.up == 0 && !moved.since_start),
            "two blocks from two providers are two allowances over one connection"
        );
    }

    #[tokio::test]
    async fn statistics_that_will_not_read_are_a_failure_rather_than_an_empty_month() {
        let refused = Sabnzbd::new(
            Fake::scripted(vec![(200, "not json")]),
            "http://127.0.0.1:8080",
            "the-key",
        )
        .moved("2026-09")
        .await;
        assert!(
            refused.is_err(),
            "a month nobody counted is not a month of nothing"
        );
    }

    /// A day, from the string a client dates one with.
    fn day(text: &str) -> Date {
        Date::parse(text).unwrap_or(Date {
            year: 1970,
            month: 1,
            day: 1,
        })
    }

    #[test]
    fn a_month_is_the_days_that_fall_in_it_and_no_others() {
        let daily = [
            (day("2026-08-31"), 1_000),
            (day("2026-09-01"), 2_000),
            (day("2026-09-30"), 3_000),
            (day("2026-10-01"), 4_000),
        ];
        assert_eq!(pulled_in(&daily, "2026-09"), 5_000);
        assert_eq!(pulled_in(&daily, "2026-08"), 1_000);
        assert_eq!(
            pulled_in(&daily, "2026-11"),
            0,
            "a month with nothing in it"
        );
    }

    #[test]
    fn a_month_is_written_the_way_the_client_dates_a_day() {
        // Zero-padded on both parts, so September is `09` and not `9` — the
        // comparison is textual and a stray digit would drop a whole month.
        assert_eq!(written(day("2026-09-04")), "2026-09");
        assert!(falls_in(day("2026-09-04"), " 2026-09 "));
        assert!(!falls_in(day("2026-09-04"), "2026-9"));
    }

    #[test]
    fn a_sum_that_would_wrap_stops_at_the_top_instead() {
        let daily = [(day("2026-09-01"), u64::MAX), (day("2026-09-02"), 1)];
        assert_eq!(pulled_in(&daily, "2026-09"), u64::MAX);
    }
}
