use super::mounts;
use bollard::models::{ContainerInspectResponse, MountPoint};
use lemonfiber_ports::docker::Mount;
use std::path::PathBuf;

/// A mount the daemon described fully is kept with both of its ends as it said them.
#[test]
fn a_described_mount_keeps_both_ends_as_the_daemon_said_them() {
    let described = ContainerInspectResponse {
        mounts: Some(vec![MountPoint {
            source: Some("/mnt/user/appdata/lemonfiber".to_owned()),
            destination: Some("/config".to_owned()),
            ..MountPoint::default()
        }]),
        ..ContainerInspectResponse::default()
    };

    assert_eq!(
        mounts(described),
        vec![Mount {
            source: PathBuf::from("/mnt/user/appdata/lemonfiber"),
            destination: PathBuf::from("/config"),
        }]
    );
}

/// A mount missing an end is left out, and so is a container with no mounts at all.
///
/// Left out rather than guessed at: a path no mount accounts for is refused by the
/// caller, which is the direction to be wrong in.
#[test]
fn a_mount_missing_either_end_is_left_out() {
    let half = |source: Option<&str>, destination: Option<&str>| MountPoint {
        source: source.map(str::to_owned),
        destination: destination.map(str::to_owned),
        ..MountPoint::default()
    };
    let described = ContainerInspectResponse {
        mounts: Some(vec![
            half(None, Some("/data")),
            half(Some("/srv"), None),
            half(Some(""), Some("/data")),
            half(Some("/srv"), Some("")),
        ]),
        ..ContainerInspectResponse::default()
    };

    assert!(mounts(described).is_empty());
    assert!(mounts(ContainerInspectResponse::default()).is_empty());
}
