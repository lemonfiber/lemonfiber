use super::{against, looked_for, EVERY};
use crate::platform::Environment;

/// Every environment, so nothing here is written for one platform and untried on
/// the rest.
const PLATFORMS: [Environment; 5] = [
    Environment::MacOs,
    Environment::LinuxNative,
    Environment::LinuxDesktop,
    Environment::Windows,
    Environment::Unsupported,
];

#[test]
fn everything_left_behind_is_named_with_a_reason_and_a_way_to_remove_it() {
    assert!(EVERY.len() >= 4, "{} entries", EVERY.len());
    for environment in PLATFORMS {
        for beside in EVERY {
            let said = against(beside, environment, false);
            assert!(
                said.by_hand.split_whitespace().count() >= 4,
                "{} on {environment:?} says nothing an operator could do: {}",
                said.what,
                said.by_hand
            );
            assert!(
                said.why.split_whitespace().count() >= 8,
                "{} on {environment:?} does not say why it is not ours",
                said.what
            );
        }
    }
}

/// The instructions are genuinely per-platform: the same entry read on macOS and
/// on Windows must not hand over the same sentence, or "platform-specific" is a
/// claim rather than a property.
#[test]
fn the_instructions_differ_between_the_platforms_they_are_written_for() {
    let same: Vec<&str> = EVERY
        .iter()
        .filter(|beside| {
            (beside.by_hand)(Environment::MacOs) == (beside.by_hand)(Environment::Windows)
        })
        .map(|beside| beside.what)
        .collect();

    assert!(
        same.is_empty(),
        "these hand a macOS operator a Windows instruction, or the reverse: {same:?}"
    );
}

/// Nothing here asks the operator to become somebody else in order to remove
/// *lemonfiber*. Where a package manager needs administrative rights that is the
/// package manager's business and is said as such, and it is never something this
/// product runs.
#[test]
fn the_binary_and_the_engine_are_told_apart_by_where_they_are_looked_for() {
    for environment in PLATFORMS {
        let binary = EVERY
            .iter()
            .find(|beside| beside.what.contains("binary"))
            .and_then(|beside| looked_for(beside, environment));
        assert_eq!(
            binary, None,
            "the binary's location is not knowable from here"
        );
    }
    assert_eq!(
        EVERY
            .first()
            .and_then(|beside| looked_for(beside, Environment::MacOs)),
        Some("/Applications/Docker.app")
    );
    assert_eq!(
        EVERY
            .first()
            .and_then(|beside| looked_for(beside, Environment::Windows)),
        None,
        "a platform with no single place worth naming is not given a made-up one"
    );
    assert_eq!(
        EVERY
            .first()
            .and_then(|beside| looked_for(beside, Environment::LinuxNative)),
        Some("/usr/bin/dockerd")
    );
}

#[test]
fn a_media_server_and_a_tunnel_are_looked_for_where_each_platform_keeps_them() {
    let jellyfin = EVERY
        .iter()
        .find(|beside| beside.what.contains("media server"));
    let tunnel = EVERY.iter().find(|beside| beside.what.contains("tunnel"));

    assert_eq!(
        jellyfin.and_then(|beside| looked_for(beside, Environment::MacOs)),
        Some("/Applications/Jellyfin.app")
    );
    assert_eq!(
        jellyfin.and_then(|beside| looked_for(beside, Environment::LinuxDesktop)),
        Some("/var/lib/jellyfin")
    );
    assert_eq!(
        jellyfin.and_then(|beside| looked_for(beside, Environment::Unsupported)),
        None
    );
    assert_eq!(
        tunnel.and_then(|beside| looked_for(beside, Environment::LinuxNative)),
        Some("/usr/sbin/tailscaled")
    );
    assert_eq!(
        tunnel.and_then(|beside| looked_for(beside, Environment::Windows)),
        None
    );
}

/// Found and not found are different reports, so an operator can tell "this is on
/// your machine" from "this is what an uninstall never touches".
#[test]
fn what_was_found_is_reported_apart_from_what_is_merely_listed() {
    let beside = EVERY.first();
    assert_eq!(
        beside.map(|beside| against(beside, Environment::MacOs, true).found),
        Some(true)
    );
    assert_eq!(
        beside.map(|beside| against(beside, Environment::MacOs, false).found),
        Some(false)
    );
}
