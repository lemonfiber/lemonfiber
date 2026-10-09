use super::{forgotten, lapsed, read, renewed, until, written, Spoken};
use crate::test_support::a_context;
use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_manifest::Date;

fn day(text: &str) -> Date {
    Date::parse(text).unwrap_or(Date {
        year: 1970,
        month: 1,
        day: 1,
    })
}

#[test]
fn a_grant_lasts_thirty_days_from_the_day_it_was_renewed() {
    assert_eq!(
        until(day("2026-10-09")).map(written).as_deref(),
        Some("2026-11-08")
    );
    assert_eq!(
        until(day("2026-12-15")).map(written).as_deref(),
        Some("2027-01-14")
    );
}

#[test]
fn a_grant_lapses_only_once_its_thirty_days_are_past() {
    let spoken: Spoken = [
        ("fresh", "2026-10-09"),
        ("last-day", "2026-09-09"),
        ("gone", "2026-09-08"),
        ("torn", "yesterday"),
    ]
    .into_iter()
    .map(|(member, said)| (member.to_owned(), said.to_owned()))
    .collect();
    assert_eq!(lapsed(&spoken, day("2026-10-09")), vec!["gone", "torn"]);
}

#[test]
fn a_renewal_is_kept_beside_the_configuration_and_a_removal_forgets_it() {
    let dir = Scratch::named("grants-kept").kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let mut ctx = a_context().build();
    ctx.settings.env_file = Some(dir.join(".env"));

    let today = renewed(&ctx, "a7f3");
    assert_eq!(read(&ctx).get("a7f3"), Some(&written(today)));
    forgotten(&ctx, "a7f3");
    forgotten(&ctx, "nobody");
    assert!(read(&ctx).is_empty());
}

/// A client that speaks twice in a day writes the file once: the second renewal finds
/// the day already kept and leaves the file as it was.
#[test]
fn a_second_renewal_on_the_same_day_leaves_the_file_alone() {
    let dir = Scratch::named("grants-same-day").kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let mut ctx = a_context().build();
    ctx.settings.env_file = Some(dir.join(".env"));
    let kept = dir.join("grants.json");
    let today = written(ctx.today());
    let as_written = format!(r#"{{"a7f3":"{today}"}}"#);
    let _ = std::fs::write(&kept, &as_written);
    let _ = renewed(&ctx, "a7f3");
    assert_eq!(std::fs::read_to_string(&kept).ok(), Some(as_written));
}

#[test]
fn with_nowhere_to_keep_them_grants_are_neither_read_nor_written() {
    let ctx = a_context().build();
    let _ = renewed(&ctx, "a7f3");
    assert!(read(&ctx).is_empty());
}
