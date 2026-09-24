use super::{key_standing, service_key};
use crate::credential::{fingerprint, Origin, State};
use std::path::Path;

/// Two values that are certainly not each other, built rather than written so a
/// scanner reads them as what they are.
fn a_pair() -> (String, String) {
    (
        format!("{}{}", "the-", "key-one"),
        format!("{}{}", "the-", "key-two"),
    )
}

#[test]
fn a_service_that_has_written_no_key_is_starting_rather_than_broken() {
    let (state, advisory) = key_standing("Sonarr", "SONARR_API_KEY", None, Some("anything"));

    assert_eq!(state, State::Absent);
    assert!(advisory.is_some_and(|said| said.contains("first start")));
}

#[test]
fn a_key_nothing_has_copied_out_yet_is_stale_and_points_at_the_seeding() {
    let (one, _) = a_pair();
    let (state, advisory) = key_standing("Sonarr", "SONARR_API_KEY", Some(&one), None);

    assert_eq!(state, State::Stale);
    let said = advisory.unwrap_or_default();
    assert!(
        said.contains("lemonfiber seed"),
        "the advisory does not point at the seeding"
    );
    assert!(
        !said.contains(&one),
        "the key nothing has copied out yet is in the advisory"
    );
}

#[test]
fn two_copies_that_agree_are_active_and_say_nothing() {
    let (one, _) = a_pair();
    let (state, advisory) = key_standing("Sonarr", "SONARR_API_KEY", Some(&one), Some(&one));

    assert_eq!(state, State::Active);
    assert_eq!(advisory, None);
}

/// The fault this comparison exists to find, and the words it has to say.
#[test]
fn a_service_that_regenerated_its_key_is_reported_against_the_stale_copy() {
    let (held, published) = a_pair();
    let (state, advisory) = key_standing("Sonarr", "SONARR_API_KEY", Some(&held), Some(&published));

    assert_eq!(state, State::Invalid);
    let said = advisory.unwrap_or_default();
    assert!(said.contains("regenerated"), "{said}");
    assert!(said.contains(&fingerprint(&held)), "{said}");
    assert!(said.contains(&fingerprint(&published)), "{said}");
}

/// The advisory names both copies and prints neither.
#[test]
fn neither_copy_appears_in_what_the_mismatch_says() {
    let (held, published) = a_pair();
    let (_, advisory) = key_standing("Sonarr", "SONARR_API_KEY", Some(&held), Some(&published));
    let said = advisory.unwrap_or_default();

    assert!(!said.is_empty());
    assert!(
        !said.contains(&held),
        "the held copy is in what the mismatch says"
    );
    assert!(
        !said.contains(&published),
        "the published copy is in what the mismatch says"
    );
}

#[test]
fn a_service_key_names_the_service_and_everything_that_reads_it_from_the_environment() {
    let (held, _) = a_pair();
    let entry = service_key(
        "Sonarr",
        "SONARR_API_KEY",
        Path::new("/stack/config/sonarr/config.xml"),
        Some(held.as_str()),
        Some(held.as_str()),
    );

    assert_eq!(entry.name, "Sonarr API key");
    assert_eq!(entry.origin, Origin::Service);
    assert_eq!(entry.consumers.len(), 2);
    assert!(entry.consumers.join(" ").contains("SONARR_API_KEY"));
    assert_eq!(entry.location, "/stack/config/sonarr/config.xml");
    assert_eq!(entry.fingerprint, Some(fingerprint(&held)));
}

#[test]
fn a_service_key_nobody_has_written_carries_no_likeness_of_a_value() {
    let entry = service_key(
        "Sonarr",
        "SONARR_API_KEY",
        Path::new("/stack/config/sonarr/config.xml"),
        None,
        None,
    );

    assert_eq!(entry.fingerprint, None);
    assert_eq!(entry.state, State::Absent);
}
