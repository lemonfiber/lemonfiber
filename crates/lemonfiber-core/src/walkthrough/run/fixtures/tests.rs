use super::TODAY;

/// The fixture clock is not older than the stack it validates against.
///
/// A service records the day its upstream last released, and validation refuses a
/// date in the future. The clock here is frozen and the manifest is a submodule
/// that moves forward, so bumping a pin to a newer release silently puts the
/// manifest ahead of the clock — and every walkthrough test then fails with a
/// fault about a service none of them is testing, which is a long way from the
/// change that caused it.
///
/// Asserted against the manifest itself rather than a date written here, so it
/// stays true as the stack moves.
#[test]
fn the_fixture_clock_is_not_older_than_the_stack_it_validates() {
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

    let today =
        lemonfiber_manifest::Date::from_unix_seconds(TODAY.as_secs().try_into().unwrap_or(0))
            .unwrap_or(lemonfiber_manifest::Date {
                year: 1970,
                month: 1,
                day: 1,
            });
    let said = format!("{:04}-{:02}-{:02}", today.year, today.month, today.day);

    assert!(
        said.as_str() >= latest.as_str(),
        "the fixture clock says {said} and the stack records a release on {latest}; \
         move TODAY forward past it — every walkthrough test fails otherwise, with a \
         fault about a service none of them is testing."
    );
}
