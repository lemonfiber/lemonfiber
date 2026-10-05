use std::path::{Path, PathBuf};

use super::{behind, container};
use crate::ports::docker::Mount;

const ID: &str = "4f1c0d7e9a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5e6f708192a3b4c5";

/// The table a container under Docker sees, trimmed to the lines that matter.
fn docker_table(root_of_hostname: &str) -> String {
    format!(
        "512 480 0:52 / / rw,relatime master:1 - overlay overlay rw,lowerdir=/x\n\
         530 512 8:1 /appdata/lemonfiber /mnt/user/appdata/lemonfiber rw - xfs /dev/md1 rw\n\
         541 512 8:1 {root_of_hostname} /etc/hostname rw,relatime - ext4 /dev/sda1 rw\n"
    )
}

#[test]
fn a_container_under_docker_is_named_by_its_hostname_mount() {
    let table = docker_table(&format!("/var/lib/docker/containers/{ID}/hostname"));

    assert_eq!(container(&table).as_deref(), Some(ID));
}

/// Unraid keeps Docker's directory on a loop device of its own, so the root of the
/// mount starts at that filesystem's top rather than at `/var/lib/docker`.
#[test]
fn a_container_whose_engine_directory_is_its_own_filesystem_is_still_named() {
    let table = docker_table(&format!("/containers/{ID}/hostname"));

    assert_eq!(container(&table).as_deref(), Some(ID));
}

#[test]
fn a_container_under_podman_is_named_by_the_same_reading() {
    let table = format!(
        "600 580 0:60 /containers/storage/overlay-containers/{ID}/userdata/hosts /etc/hosts \
         rw - xfs /dev/vda1 rw\n"
    );

    assert_eq!(container(&table).as_deref(), Some(ID));
}

/// A host's own table has no engine-written mounts, so it names no container.
#[test]
fn a_machine_that_is_not_a_container_names_none() {
    let table = "22 1 259:2 / / rw,relatime shared:1 - ext4 /dev/nvme0n1p2 rw\n\
                 25 22 0:21 / /proc rw,nosuid shared:12 - proc proc rw\n";

    assert_eq!(container(table), None);
    assert_eq!(container(""), None);
}

/// An identifier somewhere that is not an engine's containers directory, or a
/// containers directory holding something that is not an identifier, names nothing.
#[test]
fn only_an_identifier_under_a_containers_directory_counts() {
    let elsewhere = docker_table(&format!("/var/lib/backups/{ID}/hostname"));
    let short = docker_table("/var/lib/docker/containers/4f1c0d7e/hostname");
    let not_hex = docker_table(&format!(
        "/var/lib/docker/containers/{}/hostname",
        "z".repeat(64)
    ));
    let other_point = format!(
        "541 512 8:1 /var/lib/docker/containers/{ID}/hostname /srv/hostname rw - ext4 /dev/sda1 rw\n"
    );

    for table in [elsewhere, short, not_hex, other_point] {
        assert_eq!(container(&table), None, "{table}");
    }
}

/// A line too short to hold a mount point is passed over rather than read wrongly.
#[test]
fn a_truncated_line_is_passed_over() {
    assert_eq!(container("541 512 8:1"), None);
    assert_eq!(container("541 512 8:1 /containers/x"), None);
}

fn mount(source: &str, destination: &str) -> Mount {
    Mount {
        source: PathBuf::from(source),
        destination: PathBuf::from(destination),
    }
}

#[test]
fn a_path_mounted_at_its_own_path_is_the_same_path_on_the_machine() {
    let mounts = [mount(
        "/mnt/user/appdata/lemonfiber",
        "/mnt/user/appdata/lemonfiber",
    )];

    assert_eq!(
        behind(
            &mounts,
            Path::new("/mnt/user/appdata/lemonfiber/data/stack")
        ),
        Some(PathBuf::from("/mnt/user/appdata/lemonfiber/data/stack"))
    );
}

#[test]
fn a_path_mounted_somewhere_else_is_a_different_path_on_the_machine() {
    let mounts = [mount("/mnt/user/appdata/lemonfiber", "/config")];

    assert_eq!(
        behind(&mounts, Path::new("/config/data/stack")),
        Some(PathBuf::from("/mnt/user/appdata/lemonfiber/data/stack"))
    );
}

/// A mount inside another hides what was under it, so the deeper one answers.
#[test]
fn the_deepest_mount_holding_a_path_answers_for_it() {
    let mounts = [
        mount("/srv/inner", "/data/media"),
        mount("/srv/outer", "/data"),
    ];

    assert_eq!(
        behind(&mounts, Path::new("/data/media/films")),
        Some(PathBuf::from("/srv/inner/films"))
    );
    assert_eq!(
        behind(&mounts, Path::new("/data/downloads")),
        Some(PathBuf::from("/srv/outer/downloads"))
    );
}

/// A path no mount holds is in the container's own layer, which the machine has not
/// got — and a sibling whose name merely starts the same is not held by the mount.
#[test]
fn a_path_no_mount_holds_has_nothing_behind_it() {
    let mounts = [mount("/srv/data", "/data")];

    assert_eq!(
        behind(&mounts, Path::new("/home/nonroot/.local/share")),
        None
    );
    assert_eq!(behind(&mounts, Path::new("/database")), None);
    assert_eq!(behind(&[], Path::new("/data")), None);
}
