//! A profile that runs beside the forms joins a plan once the forms leave something of
//! their own, and never decides whether anything is left.

use lemonfiber_manifest::{Manifest, Profile, Protocol};

use super::super::{everything, resolve, Failure, Protocols};
use super::{named, STACK};

/// The profile these cases run beside the forms.
const BESIDE: &str = "notify";

/// The committed stack with its profile running beside the forms declared as needing
/// `protocol`.
fn beside(protocol: Option<Protocol>) -> Option<Manifest> {
    let mut manifest = Manifest::from_toml(STACK).ok()?;
    manifest.profiles.retain(|profile| profile.id != BESIDE);
    manifest.profiles.push(Profile {
        id: BESIDE.to_owned(),
        name: "Notifications".to_owned(),
        description: "A self-hosted push channel".to_owned(),
        protocol,
        beside: true,
    });
    Some(manifest)
}

#[test]
fn a_form_with_something_of_its_own_left_runs_the_beside_profile_with_it() {
    let plan = beside(None)
        .and_then(|manifest| resolve(&manifest, &named(&["library"]), Protocols::none()).ok());

    assert_eq!(
        plan.map(|plan| plan.profiles.into_iter().collect::<Vec<_>>()),
        Some(named(&["media", BESIDE]))
    );
}

#[test]
fn a_beside_profile_never_leaves_a_form_something_to_run() {
    let refused = beside(None)
        .and_then(|manifest| resolve(&manifest, &named(&["dl"]), Protocols::none()).err());

    assert!(
        matches!(refused, Some(Failure::NothingLeft { .. })),
        "{refused:?}"
    );
}

#[test]
fn everything_runs_the_beside_profile_once_something_of_the_stack_is_left() {
    let plan = beside(None).and_then(|manifest| everything(&manifest, Protocols::none()).ok());

    assert!(
        plan.is_some_and(|plan| plan.profiles.contains(BESIDE)),
        "the beside profile was left out of everything"
    );
}

#[test]
fn a_beside_profile_needing_a_provider_nobody_configured_is_left_out_without_narrowing_the_form() {
    let usenet_only = Protocols {
        usenet: true,
        torrent: false,
    };
    let plan = beside(Some(Protocol::Torrent))
        .and_then(|manifest| resolve(&manifest, &named(&["library"]), usenet_only).ok());

    assert!(
        plan.as_ref()
            .is_some_and(|plan| !plan.profiles.contains(BESIDE)
                && plan.dropped.iter().all(|out| out.profile != BESIDE)),
        "{plan:?}"
    );
}
