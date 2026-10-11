use super::{held_back, Limits};
use crate::ports::service::{Access, Certificate, Failure, Unrated};

/// The media server's own table, as a British house's server answers it.
fn table() -> Vec<Certificate> {
    [("U", 0), ("PG", 8), ("12A", 12), ("15", 15), ("18", 18)]
        .into_iter()
        .map(|(name, age)| Certificate {
            name: name.to_owned(),
            age,
        })
        .collect()
}

/// An account held to `age` with `unrated` content treated as it says.
fn access(age: Option<u32>, unrated: Unrated) -> Access {
    Access {
        every_library: true,
        age_limit: age,
        unrated,
        ..Access::default()
    }
}

/// A server that did not answer.
fn silent() -> Failure {
    Failure::Unavailable {
        service: "media server".to_owned(),
    }
}

/// Which of `certifications` the limits an account is held to admit, read against the
/// British table; nothing where no limits could be read.
fn admitted(
    age: Option<u32>,
    unrated: Unrated,
    certifications: &[Option<&str>],
) -> Option<Vec<bool>> {
    Limits::read(
        &access(age, unrated),
        Ok(table()),
        Ok(Some("GB".to_owned())),
    )
    .map(|limits| {
        certifications
            .iter()
            .map(|certification| limits.admits(*certification))
            .collect()
    })
}

#[test]
fn a_member_held_to_twelve_is_offered_what_is_rated_twelve_or_under_and_nothing_older() {
    let limits = Limits::read(
        &access(Some(12), Unrated::HeldBack),
        Ok(table()),
        Ok(Some("GB".to_owned())),
    );
    assert_eq!(
        limits
            .as_ref()
            .map(|limits| (limits.restricted(), limits.region())),
        Some((true, "GB"))
    );
    assert_eq!(
        admitted(
            Some(12),
            Unrated::HeldBack,
            &[
                Some("12A"),
                Some("12a"),
                Some(" PG "),
                Some("15"),
                Some("18")
            ]
        ),
        Some(vec![true, true, true, false, false])
    );
}

#[test]
fn a_title_with_no_certification_is_decided_by_the_unrated_policy() {
    assert_eq!(
        admitted(Some(12), Unrated::HeldBack, &[None]),
        Some(vec![false])
    );
    assert_eq!(
        admitted(Some(12), Unrated::LetThrough, &[None]),
        Some(vec![true])
    );
}

#[test]
fn a_certification_the_table_does_not_name_is_held_back_whatever_the_unrated_policy() {
    assert_eq!(
        admitted(Some(18), Unrated::LetThrough, &[Some("PG-13"), Some("18")]),
        Some(vec![false, true])
    );
}

#[test]
fn a_member_who_holds_back_only_the_unrated_is_offered_every_rated_title() {
    assert_eq!(
        admitted(None, Unrated::HeldBack, &[Some("18"), None]),
        Some(vec![true, false])
    );
}

#[test]
fn a_member_held_to_a_limit_is_offered_nothing_where_the_table_cannot_be_read() {
    let held = access(Some(12), Unrated::LetThrough);
    assert_eq!(
        Limits::read(&held, Err(silent()), Ok(Some("GB".to_owned()))),
        None
    );
    assert_eq!(
        Limits::read(&held, Ok(Vec::new()), Ok(Some("GB".to_owned()))),
        None
    );
}

#[test]
fn a_member_held_to_a_limit_is_offered_nothing_where_the_country_is_unknown() {
    let held = access(Some(12), Unrated::LetThrough);
    assert_eq!(Limits::read(&held, Ok(table()), Err(silent())), None);
    assert_eq!(Limits::read(&held, Ok(table()), Ok(None)), None);
}

#[test]
fn a_member_nothing_holds_back_is_offered_everything_and_nothing_is_looked_up() {
    let open = Limits::open();
    assert!(!held_back(&access(None, Unrated::LetThrough)));
    assert!(held_back(&access(Some(15), Unrated::LetThrough)));
    assert!(held_back(&access(None, Unrated::HeldBack)));
    assert!(!open.restricted());
    assert_eq!(open.region(), "");
    assert!(open.admits(None));
}
