use super::{crowded, host_side, is_beneath_the_root};
use std::path::PathBuf;

/// The stack's compose files, as the check is given them.
fn given(files: &[(&str, &str)]) -> Vec<(PathBuf, String)> {
    files
        .iter()
        .map(|(path, text)| (PathBuf::from(path), (*text).to_owned()))
        .collect()
}

/// A one-service compose file declaring the given volume entries.
///
/// Built rather than written out, so the clean cases and the refused ones
/// share a shape: a builder that stopped describing a compose file would fail
/// the tests that expect a refusal instead of quietly passing the ones that
/// expect none.
fn service_with(volumes: &[&str]) -> String {
    let mut text = String::from("services:\n  sonarr:\n    volumes:\n");
    for volume in volumes {
        text.push_str("      - ");
        text.push_str(volume);
        text.push('\n');
    }
    text
}

/// The services reported crowded in a single built file.
fn crowding(volumes: &[&str]) -> Vec<String> {
    named(&[("tv.yml", &service_with(volumes))])
}

/// The services reported crowded, by name.
fn named(files: &[(&str, &str)]) -> Vec<String> {
    crowded(&given(files))
        .into_iter()
        .map(|crowded| crowded.service)
        .collect()
}

#[test]
fn the_sanctioned_shape_passes() {
    // One mount of the data root, and a configuration directory that is not
    // beneath it. This is what every service in the shipped stack looks like.
    assert!(crowding(&["${DATA_ROOT:-./data}:/data", "./config/sonarr:/config"]).is_empty());
}

#[test]
fn two_mounts_beneath_the_root_are_refused() {
    // The tidier-looking form, and the one that silently turns every import
    // into a copy: /downloads and /media are different filesystems inside the
    // container, so nothing can be linked between them.
    assert_eq!(
        crowding(&[
            "${DATA_ROOT}/downloads:/downloads",
            "${DATA_ROOT}/media:/media"
        ]),
        vec!["sonarr".to_owned()]
    );
}

#[test]
fn what_it_costs_is_named_rather_than_the_rule_it_broke() {
    // Naming the rule it broke tells an operator nothing. What they need is
    // what will happen to them, and which paths did it.
    let file = service_with(&[
        "${DATA_ROOT}/downloads:/downloads",
        "${DATA_ROOT}/media:/media",
    ]);
    let said = crowded(&given(&[("tv.yml", &file)]))
        .first()
        .map(ToString::to_string)
        .unwrap_or_default();
    assert!(said.contains("copied rather than hardlinked"), "{said}");
    assert!(said.contains("${DATA_ROOT}/media:/media"), "{said}");
}

#[test]
fn two_services_with_one_mount_each_are_the_required_form() {
    // Every service mounting the whole data root once is exactly right, and a
    // rule counted across the stack rather than per service would refuse it.
    let stack = "
services:
  sonarr:
    volumes:
      - ${DATA_ROOT}:/data
  qbittorrent:
    volumes:
      - ${DATA_ROOT}:/data
";
    assert!(named(&[("media.yml", stack)]).is_empty());
}

#[test]
fn a_mount_inherited_through_extends_still_counts() {
    // The case a file-at-a-time reading calls clean: one mount here, one in
    // what it extends, and two in the container that actually runs.
    let common = "
services:
  defaults:
    volumes:
      - ${DATA_ROOT}/downloads:/downloads
";
    let tv = "
services:
  sonarr:
    extends:
      file: compose/_common.yml
      service: defaults
    volumes:
      - ${DATA_ROOT}/media:/media
";
    assert_eq!(
        named(&[("compose/_common.yml", common), ("compose/tv.yml", tv)]),
        vec!["sonarr".to_owned()]
    );
}

#[test]
fn a_service_extending_one_in_its_own_file_is_followed_too() {
    let stack = "
services:
  base:
    volumes:
      - ${DATA_ROOT}/downloads:/downloads
  sonarr:
    extends: base
    volumes:
      - ${DATA_ROOT}/media:/media
";
    assert_eq!(named(&[("tv.yml", stack)]), vec!["sonarr".to_owned()]);
}

#[test]
fn services_extending_each_other_are_refused_rather_than_looped_over() {
    // Compose would refuse this stack; this has no business hanging on it.
    let cycle = "
services:
  one:
    extends: two
    volumes:
      - ${DATA_ROOT}/a:/a
  two:
    extends: one
    volumes:
      - ${DATA_ROOT}/b:/b
";
    assert_eq!(
        named(&[("loop.yml", cycle)]),
        vec!["one".to_owned(), "two".to_owned()]
    );
}

#[test]
fn an_anchor_is_read_the_way_compose_reads_it() {
    // A stack written with YAML anchors rather than `extends`. Without the
    // merge applied, the service would read as having no volumes at all and
    // this check would call it clean.
    let anchored = "
x-shared: &shared
  volumes:
    - ${DATA_ROOT}/downloads:/downloads
    - ${DATA_ROOT}/media:/media
services:
  sonarr:
    <<: *shared
";
    assert_eq!(named(&[("tv.yml", anchored)]), vec!["sonarr".to_owned()]);
}

#[test]
fn the_long_form_is_read_as_well_as_the_short() {
    let long = "
services:
  sonarr:
    volumes:
      - type: bind
        source: ${DATA_ROOT}/downloads
        target: /downloads
      - type: bind
        source: ${DATA_ROOT}/media
        target: /media
";
    assert_eq!(named(&[("tv.yml", long)]), vec!["sonarr".to_owned()]);
}

#[test]
fn a_named_volume_is_not_a_path_beneath_anything() {
    // Caddy's certificate store, and the reason the test is on the data root
    // rather than on how many volumes a service has.
    assert!(crowding(&["caddy_data:/data", "${DATA_ROOT}:/srv"]).is_empty());
    // And the same file with a second path beneath the root is refused, so a
    // pass above cannot come from a parser that read nothing.
    assert_eq!(
        crowding(&["caddy_data:/data", "${DATA_ROOT}:/srv", "${DATA_ROOT}/x:/x"]),
        vec!["sonarr".to_owned()]
    );
}

#[test]
fn a_variable_that_merely_starts_the_same_is_not_the_data_root() {
    // DATA_ROOT_BACKUP is a different location, and reading it as the data
    // root would refuse a stack that is doing nothing wrong.
    assert!(is_beneath_the_root("${DATA_ROOT}"));
    assert!(is_beneath_the_root("${DATA_ROOT:-./data}/media"));
    assert!(is_beneath_the_root("$DATA_ROOT/media"));
    assert!(!is_beneath_the_root("${DATA_ROOT_BACKUP}/media"));
    assert!(!is_beneath_the_root("./config/sonarr"));
    assert!(!is_beneath_the_root("caddy_data"));
}

#[test]
fn a_default_carrying_a_colon_does_not_cut_the_host_path_short() {
    // `${DATA_ROOT:-./data}` has a colon of its own, and splitting at the
    // first one leaves `${DATA_ROOT` — which matches nothing, so every stack
    // written the shipped way would read as clean.
    assert_eq!(
        host_side("${DATA_ROOT:-./data}:/data"),
        "${DATA_ROOT:-./data}"
    );
    assert_eq!(host_side("./config:/config:ro"), "./config");
    assert_eq!(host_side("caddy_data"), "caddy_data");
}

#[test]
fn a_file_that_is_not_a_compose_file_is_passed_over() {
    // Stack directories hold licences, readmes and justfiles. A parser given
    // one must not refuse the stack over it.
    assert!(named(&[("README.md", "# not yaml: [")]).is_empty());
    assert!(named(&[("stack.toml", "schema_version = 1")]).is_empty());
}

#[test]
fn a_service_that_mounts_nothing_is_not_reported() {
    let bare = "
services:
  watchtower:
    image: containrrr/watchtower
";
    assert!(named(&[("tuning.yml", bare)]).is_empty());
}

#[test]
fn extending_something_no_file_declares_adds_nothing_rather_than_a_guess() {
    // A stack can name a parent this never sees — a typo, or a file outside
    // what was handed here. What it declares itself still counts; what it
    // extends contributes nothing, because inventing volumes for it would
    // refuse a stack over something nobody wrote.
    let orphan = "
services:
  sonarr:
    extends:
      file: somewhere/else.yml
      service: nothing-declares-this
    volumes:
      - ${DATA_ROOT}:/data
";
    assert!(named(&[("tv.yml", orphan)]).is_empty());
}
