use std::path::Path;

use super::{moving, unresolvable, Existing};

/// A path a service holds, with whether the host directory behind it is there.
fn holding(service: &str, path: &str, present: bool) -> Existing {
    Existing {
        service: service.to_owned(),
        path: path.to_owned(),
        present,
    }
}

#[test]
fn a_folder_the_new_location_already_holds_is_carried_and_says_where_to() {
    let moved = moving(
        &[holding("sonarr", "/data/media/tv", true)],
        Path::new("/srv/new"),
    );
    let one = moved.first();
    assert!(one.is_some_and(|row| row.carried));
    assert_eq!(
        one.and_then(|row| row.host.clone()),
        Some("/srv/new/media/tv".to_owned())
    );
    assert!(unresolvable(&moved).is_none());
}

#[test]
fn a_folder_the_new_location_does_not_hold_is_refused_and_names_the_host_path() {
    let moved = moving(
        &[holding("radarr", "/data/media/movies", false)],
        Path::new("/srv/new"),
    );
    let said = moved
        .first()
        .map(|row| row.because.clone())
        .unwrap_or_default();
    assert!(
        said.contains("/srv/new/media/movies is not there"),
        "{said}"
    );
    let refusal = unresolvable(&moved).unwrap_or_default();
    assert!(refusal.contains("/data/media/movies"), "{refusal}");
    assert!(refusal.contains("Nothing was written."), "{refusal}");
}

#[test]
fn a_folder_outside_the_mount_cannot_be_repointed_by_a_move_at_all() {
    // An adopted stack's own root folder, or one the operator set by hand. No
    // write to the data location reaches it, so the move is refused rather than
    // silently leaving it where it was.
    let moved = moving(
        &[holding("sonarr", "/mnt/old/tv", true)],
        Path::new("/srv/new"),
    );
    let row = moved.first();
    assert!(row.is_some_and(|row| !row.carried && row.host.is_none()));
    let said = row.map(|row| row.because.clone()).unwrap_or_default();
    assert!(said.contains("outside /data"), "{said}");
    assert!(unresolvable(&moved).is_some());
}

#[test]
fn the_mount_itself_is_repointed_like_anything_beneath_it() {
    let moved = moving(&[holding("lidarr", "/data/", true)], Path::new("/srv/new"));
    assert_eq!(
        moved.first().and_then(|row| row.host.clone()),
        Some("/srv/new".to_owned())
    );
}

#[test]
fn a_name_that_merely_starts_with_the_mount_is_not_under_it() {
    // `/database` is not inside `/data`, and reading it as though it were would
    // carry a move that must be refused.
    let moved = moving(
        &[holding("sonarr", "/database/tv", true)],
        Path::new("/srv/new"),
    );
    assert!(moved.first().is_some_and(|row| row.host.is_none()));
}

#[test]
fn a_stack_holding_no_library_path_has_nothing_to_refuse() {
    assert!(unresolvable(&moving(&[], Path::new("/srv/new"))).is_none());
}
