use std::collections::BTreeSet;
use std::path::PathBuf;

use super::attached;

/// A service's networks as a set, for comparing.
fn set(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

/// Each service is on the networks it names in either syntax across every file declaring
/// it, on the default one where none names any, and on none where it takes another
/// container's network — and a file declaring it without networks takes none away.
#[test]
fn each_service_is_on_what_it_names_the_default_or_nothing() {
    let files =
        vec![
        (
            PathBuf::from("compose/a.yml"),
            "services:\n  listed:\n    networks: [default, inner]\n  keyed:\n    networks:\n      \
             inner: {}\n  plain:\n    image: x\n  borrowed:\n    network_mode: \"service:plain\"\n"
                .to_owned(),
        ),
        (PathBuf::from("compose/b.yml"), "not: [a, compose".to_owned()),
        (
            PathBuf::from("stacks/override.yml"),
            "services:\n  listed:\n    volumes: [\"/x:/x\"]\n  keyed:\n    networks: [outer]\n"
                .to_owned(),
        ),
    ];

    let found = attached(&files);

    assert_eq!(found.get("listed"), Some(&set(&["default", "inner"])));
    assert_eq!(found.get("keyed"), Some(&set(&["inner", "outer"])));
    assert_eq!(found.get("plain"), Some(&set(&["default"])));
    assert_eq!(found.get("borrowed"), Some(&set(&[])));
}

/// The stack this build ships keeps the request gate's upstreams on a network of their
/// own, which is what a plugin standing in for one of them has to join.
#[test]
fn the_shipped_stack_keeps_the_gates_upstreams_apart() {
    let found = crate::test_support::stack().attached();

    let gate_upstream = |id: &str| {
        found
            .get(id)
            .is_some_and(|networks| networks.contains("gate-upstream"))
    };
    assert!(gate_upstream("sonarr"));
    assert!(gate_upstream("radarr"));
    assert!(gate_upstream("jellyfin"));
    assert!(!gate_upstream("lidarr"));
    assert_eq!(found.get("qbittorrent"), Some(&set(&[])));
}

/// A plugin's own entry is never read as the stack's statement of who reaches whom: a
/// stack service named again in a plugin's file keeps only the networks the stack's
/// files put it on.
#[test]
fn a_plugins_entry_adds_nothing_to_what_the_stack_puts_a_service_on() {
    let files = vec![
        (
            PathBuf::from("/srv/stack/compose/tv.yml"),
            "services:\n  sonarr:\n    networks: [default]\n".to_owned(),
        ),
        (
            PathBuf::from("/srv/stack/compose/plugins/brought.yml"),
            "services:\n  sonarr:\n    networks: [default, gate-upstream]\n  brought:\n    \
             networks: [default, gate-upstream]\n"
                .to_owned(),
        ),
    ];

    let found = attached(&files);

    assert_eq!(found.get("sonarr"), Some(&set(&["default"])));
    assert_eq!(found.get("brought"), None);
}
