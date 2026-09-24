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
