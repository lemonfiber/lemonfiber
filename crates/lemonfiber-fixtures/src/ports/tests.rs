use super::TODAY;

/// The frozen day is not older than the stack it validates against.
///
/// Every test that builds a context over the real manifest validates it against
/// this day, and a service records the day its upstream last released. Bumping a
/// pin to a newer release silently puts the manifest ahead of the clock, and the
/// tests then fail with a fault about a service none of them is testing — a long
/// way from the change that caused it.
///
/// Read out of the manifest rather than written down here, so it stays true as
/// the stack moves.
#[test]
fn the_frozen_day_is_not_older_than_the_stack_it_validates() {
    let manifest = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/media-stack/stack.toml"
    ));
    let latest = manifest
        .lines()
        .filter_map(|line| line.trim().strip_prefix("last_release = "))
        .map(|value| value.trim().trim_matches('"').to_owned())
        .max()
        .unwrap_or_default();

    // Days from the civil date, so this needs no calendar crate: the manifest's
    // dates and the frozen day become one number each, and the comparison is
    // whether one is behind the other.
    let (year, rest) = latest.split_at(4);
    let released = days_from_civil(
        year.parse().unwrap_or(0),
        rest.get(1..3).and_then(|m| m.parse().ok()).unwrap_or(1),
        rest.get(4..6).and_then(|d| d.parse().ok()).unwrap_or(1),
    );
    let frozen = i64::try_from(TODAY / 86_400).unwrap_or(0);

    assert!(
        frozen >= released,
        "the frozen day is {frozen} days after the epoch and the stack records a \
         release on {latest}, which is {released}; move TODAY forward past it — the \
         tests that validate the manifest fail otherwise, with a fault about a \
         service none of them is testing."
    );
}

/// Days from `1970-01-01` to a civil date — Howard Hinnant's algorithm, which is
/// the short exact one and needs nothing but integer arithmetic.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}
