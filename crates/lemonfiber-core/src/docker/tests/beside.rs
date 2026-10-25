use lemonfiber_manifest::Criticality;

use super::{container, manifest};
use crate::config::Protocols;
use crate::docker::{survey_beside, undeclared_beside, Halted, State};
use crate::plugin::Profiled;
use crate::ports::docker::{Health, Lifecycle};

/// The services the stack's `library` form brings.
const LIBRARY: [&str; 8] = [
    "jellyfin",
    "door",
    "seerr",
    "request-gate",
    "decline",
    "calibre-web-automated",
    "audiobookshelf",
    "navidrome",
];

/// A service of a plugin the bundle installs, in the stack profile `profile`.
fn beside(id: &str, profile: &str) -> Profiled {
    Profiled {
        id: id.to_owned(),
        name: format!("{id} moved in"),
        describes: format!("what {id} is for"),
        profile: profile.to_owned(),
        criticality: Criticality::Important,
    }
}

/// A moved service the stack does not declare is surveyed as one of its profile's, up
/// for the forms that bring that profile; one under a declared id stays the stack's
/// service; and neither is a container the stack never declared.
#[test]
fn a_moved_service_is_surveyed_with_the_profile_it_sits_in() {
    let moved = [
        beside("jellyfin-adapter", "media"),
        beside("jellyfin", "media"),
    ];
    let running: Vec<_> = LIBRARY
        .iter()
        .chain(["jellyfin-adapter"].iter())
        .map(|id| container(id, Lifecycle::Running, Health::Healthy))
        .collect();
    let Some(manifest) = manifest() else {
        unreachable!("the carried stack parses");
    };
    let media = vec!["media".to_owned()];

    let seen = survey_beside(
        &manifest,
        &moved,
        &media,
        &running,
        &Halted::default(),
        Protocols::both(),
    );

    let adapter = seen.iter().find(|one| one.id == "jellyfin-adapter");
    assert!(adapter.is_some_and(|one| one.state == State::Healthy
        && one.criticality == Criticality::Important
        && one.forms.contains(&"library".to_owned())));
    let jellyfin: Vec<_> = seen.iter().filter(|one| one.id == "jellyfin").collect();
    assert!(
        matches!(jellyfin.as_slice(), [one] if one.name != "jellyfin moved in"),
        "{jellyfin:?}"
    );
    assert!(undeclared_beside(&manifest, &moved, &running, &Halted::default()).is_empty());
}

/// A moved service nobody started is absent from its profile, not missing from it.
#[test]
fn a_moved_service_nobody_started_is_absent() {
    let Some(manifest) = manifest() else {
        unreachable!("the carried stack parses");
    };

    let seen = survey_beside(
        &manifest,
        &[beside("jellyfin-adapter", "media")],
        &["media".to_owned()],
        &[],
        &Halted::default(),
        Protocols::both(),
    );

    assert!(seen.iter().any(|one| one.id == "jellyfin-adapter"
        && one.state == State::Absent
        && one.forms.is_empty()));
}
