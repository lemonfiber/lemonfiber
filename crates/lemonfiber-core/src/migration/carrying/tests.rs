use super::{carrying, not_carried};
use crate::migration::tests::{image, ours};

/// What adopting one service came to, as (verdict, backup first, refused).
fn taking(existing: &str, pinned: &str) -> Option<(String, bool, bool)> {
    let images = [image(
        &[&format!("linuxserver/sonarr:{existing}")],
        &["media"],
    )];
    let ours = [ours("sonarr", "linuxserver/sonarr", pinned, None)];
    carrying(&images, "media", &ours)
        .first()
        .map(|read| (read.verdict.clone(), read.backup_first, read.refused))
}

#[test]
fn the_version_already_here_being_ours_is_taken_over_as_it_stands() {
    assert_eq!(
        taking("4.0.1", "4.0.1"),
        Some(("same".to_owned(), false, false))
    );
}

#[test]
fn an_older_database_is_upgraded_and_backed_up_before_anything_opens_it() {
    assert_eq!(
        taking("4.0.1", "4.0.2"),
        Some(("upgrade".to_owned(), true, false))
    );
}

#[test]
fn a_database_newer_than_ours_is_refused_rather_than_attempted() {
    assert_eq!(
        taking("4.0.2", "4.0.1"),
        Some(("downgrade".to_owned(), false, true))
    );
}

#[test]
fn versions_that_cannot_be_ordered_are_backed_up_rather_than_assumed_safe() {
    assert_eq!(
        taking("latest", "4.0.1"),
        Some(("cannot tell".to_owned(), true, false))
    );
}

#[test]
fn a_refusal_names_both_versions_so_it_can_be_argued_with() {
    let images = [image(&["linuxserver/sonarr:4.0.9"], &["media"])];
    let ours = [ours("sonarr", "linuxserver/sonarr", "4.0.1", None)];
    let said = carrying(&images, "media", &ours)
        .first()
        .map(|read| read.because.clone())
        .unwrap_or_default();
    assert!(said.contains("4.0.9"), "{said}");
    assert!(said.contains("4.0.1"), "{said}");
}

#[test]
fn a_service_we_run_that_is_not_on_this_project_is_not_reported_as_carried() {
    let images = [image(&["linuxserver/sonarr:4.0.1"], &["other"])];
    let ours = [ours("sonarr", "linuxserver/sonarr", "4.0.1", None)];
    assert!(carrying(&images, "media", &ours).is_empty());
}

#[test]
fn what_would_be_carried_reads_in_a_settled_order() {
    let images = [
        image(&["linuxserver/sonarr:4.0.1"], &["media"]),
        image(&["linuxserver/radarr:5.0.1"], &["media"]),
    ];
    let ours = [
        ours("sonarr", "linuxserver/sonarr", "4.0.1", None),
        ours("radarr", "linuxserver/radarr", "5.0.1", None),
    ];
    let order: Vec<String> = carrying(&images, "media", &ours)
        .into_iter()
        .map(|read| read.service)
        .collect();
    assert_eq!(order, vec!["radarr".to_owned(), "sonarr".to_owned()]);
}

#[test]
fn what_never_carries_across_is_named_rather_than_left_to_be_discovered() {
    let named = not_carried();
    assert!(named.len() >= 4, "{named:?}");
    let all: String = named.iter().map(|item| item.what.clone()).collect();
    assert!(all.contains("custom formats"), "{all}");
}
