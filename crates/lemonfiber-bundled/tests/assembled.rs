//! The stack the bundle assembles, held to the media stack it replaces: every service
//! the same in every field, but for the differences named here until each is levelled.

use std::collections::BTreeSet;
use std::path::Path;

use toml::{Table, Value};

const CATALOGUE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/plugins");
const MEDIA_STACK: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/media-stack");

/// Services the media stack runs that the bundle does not assemble yet.
const NOT_ASSEMBLED: [&str; 4] = ["decline", "jellyfin", "recyclarr", "request-gate"];

/// Fields of a service that differ between the two, by service and field: the guarded
/// door and the proxy run the images lemonfiber builds for them (ADR-0045).
const DIFFERING: [(&str, &str); 6] = [
    ("caddy", "digest"),
    ("caddy", "image"),
    ("caddy", "tag"),
    ("door", "digest"),
    ("door", "image"),
    ("door", "tag"),
];

fn services(text: &str) -> Vec<Table> {
    let parsed: Table = toml::from_str(text).unwrap_or_default();
    parsed
        .get("service")
        .and_then(Value::as_array)
        .map(|services| {
            services
                .iter()
                .filter_map(|one| one.as_table().cloned())
                .collect()
        })
        .unwrap_or_default()
}

fn id(service: &Table) -> &str {
    service
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
}

fn assembled() -> String {
    let root = Path::new(CATALOGUE);
    let text = std::fs::read_to_string(root.join(lemonfiber_bundle::FILE)).unwrap_or_default();
    let Ok((_, held)) = lemonfiber_bundle::read(root) else {
        unreachable!("the embedded bundle reads; the build refuses one that does not");
    };
    lemonfiber_bundle::assembled(&text, &held, env!("CARGO_PKG_VERSION")).unwrap_or_default()
}

fn media_stack() -> String {
    lemonfiber_manifest::read(Path::new(MEDIA_STACK)).unwrap_or_default()
}

#[test]
fn the_assembled_stack_reads_as_a_stack_manifest() {
    assert!(lemonfiber_manifest::Manifest::from_toml(&assembled()).is_ok());
}

#[test]
fn every_service_the_media_stack_runs_is_assembled_but_those_named() {
    let ours: BTreeSet<String> = services(&assembled())
        .iter()
        .map(|one| id(one).to_owned())
        .collect();
    let missing: BTreeSet<String> = services(&media_stack())
        .iter()
        .map(|one| id(one).to_owned())
        .filter(|id| !ours.contains(id))
        .collect();
    let named: BTreeSet<String> = NOT_ASSEMBLED.iter().map(|id| (*id).to_owned()).collect();
    assert_eq!(missing, named);
}

#[test]
fn every_assembled_service_is_the_media_stacks_but_where_named() {
    let ours = services(&assembled());
    let mut differing = BTreeSet::new();
    for theirs in services(&media_stack()) {
        let Some(mine) = ours.iter().find(|one| id(one) == id(&theirs)) else {
            continue;
        };
        for key in theirs.keys().chain(mine.keys()) {
            if theirs.get(key) != mine.get(key) {
                differing.insert((id(&theirs).to_owned(), key.clone()));
            }
        }
    }
    let named: BTreeSet<(String, String)> = DIFFERING
        .iter()
        .map(|(id, key)| ((*id).to_owned(), (*key).to_owned()))
        .collect();
    assert_eq!(differing, named);
}
