use super::{outside_compose, repository, standing_on, version_of};
use crate::migration::tests::{image, ours};

#[test]
fn a_version_is_dropped_to_leave_the_repository() {
    assert_eq!(repository("plex:1.2"), "plex");
    assert_eq!(version_of("plex:1.2"), "1.2");
}

#[test]
fn an_image_named_without_a_version_is_all_repository() {
    assert_eq!(repository("plex"), "plex");
    assert_eq!(version_of("plex"), "plex");
}

#[test]
fn a_registry_carrying_its_own_port_is_not_read_as_a_version() {
    assert_eq!(
        repository("example.test:5000/plex:1.2"),
        "example.test:5000/plex"
    );
    assert_eq!(
        repository("example.test:5000/plex"),
        "example.test:5000/plex"
    );
}

#[test]
fn a_container_started_by_hand_is_named_from_the_image_beneath_it() {
    let images = [image(&["plex:latest"], &[""])];
    let named = outside_compose(&images, &[ours("plex", "plex", "1.0", None)]);
    let what = named.first().map(|item| item.what.clone());
    assert_eq!(what, Some("plex:latest".to_owned()), "{named:?}");
}

#[test]
fn an_image_only_projects_stand_on_is_not_named_as_unsupported() {
    let images = [image(&["plex:latest"], &["media"])];
    let named = outside_compose(&images, &[ours("plex", "plex", "1.0", None)]);
    assert!(named.is_empty(), "{named:?}");
}

#[test]
fn an_image_we_do_not_run_is_not_a_migration_finding() {
    let images = [image(&["a-database:17"], &[""])];
    let named = outside_compose(&images, &[ours("plex", "plex", "1.0", None)]);
    assert!(named.is_empty(), "{named:?}");
}

#[test]
fn what_was_started_outside_compose_reads_in_a_settled_order() {
    let images = [image(&["sonarr:1"], &[""]), image(&["plex:2"], &[""])];
    let ours = [
        ours("sonarr", "sonarr", "1", None),
        ours("plex", "plex", "2", None),
    ];
    let named: Vec<String> = outside_compose(&images, &ours)
        .into_iter()
        .map(|item| item.what)
        .collect();
    assert_eq!(named, vec!["plex:2".to_owned(), "sonarr:1".to_owned()]);
}

#[test]
fn a_service_standing_on_a_project_answers_with_its_version() {
    let images = [image(&["linuxserver/sonarr:4.0.1"], &["media"])];
    assert_eq!(
        standing_on(&images, "media", "linuxserver/sonarr"),
        Some("4.0.1".to_owned())
    );
}

#[test]
fn a_service_on_another_project_is_not_standing_on_this_one() {
    let images = [image(&["linuxserver/sonarr:4.0.1"], &["other"])];
    assert_eq!(standing_on(&images, "media", "linuxserver/sonarr"), None);
}
