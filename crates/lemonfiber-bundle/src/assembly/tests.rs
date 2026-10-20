use std::collections::BTreeMap;

use lemonfiber_manifest::Manifest;

use super::assembled;
use crate::{Held, Pin, Stage};

const BUNDLE: &str = r#"
schema = 2

[[profile]]
id = "tv"
name = "Television"
description = "Automated television acquisition"

[[form]]
id = "tv"
name = "Television"
description = "Watch for episodes"
profiles = ["tv"]

[[wiring]]
by = "curator"
asks = "indexer.search"
filled_by = "finder"
why = "The stack's choice of the two that answer."
"#;

const CURATOR: &str = r#"
schema_version = 1

[plugin]
id          = "curator"
name        = "Curator"
version     = "1.0.0"
description = "Watches for new episodes and fetches them"
without_it  = "Find and download episodes yourself"
upstream    = "https://example.invalid/curator"
license     = "GPL-3.0-only"
forms       = ["tv"]

[[service]]
id          = "curator"
name        = "Curator"
image       = "example.invalid/curator"
digest      = "sha256:0000000000000000000000000000000000000000000000000000000000000000"
tag         = "1.0.0"
criticality = "core"
memory_mib  = 300
reaches     = ""
asks_for    = "Nothing beyond this machine."
replaces    = "curator"

[[ask]]
capability = "download.usenet"

[[ask]]
capability = "indexer.search"

[[claim]]
capability = "library.curate"
"#;

fn held(id: &str, copy: &str, stage: Stage) -> Held {
    Held {
        pin: Pin {
            id: id.into(),
            origin: format!("https://example.invalid/plugin-{id}"),
            release: None,
            revision: "0".repeat(40),
            manifest: String::new(),
            tree: None,
            fills: Vec::new(),
            stage: Some(stage),
            profile: Some("tv".into()),
            last_release: Some("2026-10-01".into()),
        },
        copy: copy.into(),
        tree: BTreeMap::new(),
    }
}

fn stack(held: &[Held]) -> toml::Table {
    let Ok(text) = assembled(BUNDLE, held, "0.18.0") else {
        unreachable!("the fixture assembles");
    };
    toml::from_str(&text).unwrap_or_default()
}

fn only_service(stack: &toml::Table) -> toml::Table {
    stack
        .get("service")
        .and_then(toml::Value::as_array)
        .and_then(|services| services.first())
        .and_then(toml::Value::as_table)
        .cloned()
        .unwrap_or_default()
}

#[test]
fn an_assembled_part_is_its_service_with_the_pins_profile_and_its_plugins_words() {
    let service = only_service(&stack(&[held("curator", CURATOR, Stage::Assembled)]));
    let field = |name: &str| service.get(name).and_then(toml::Value::as_str);
    assert_eq!(field("profile"), Some("tv"));
    assert_eq!(field("last_release"), Some("2026-10-01"));
    assert_eq!(
        field("describes"),
        Some("Watches for new episodes and fetches them")
    );
    assert_eq!(field("license"), Some("GPL-3.0-only"));
    assert_eq!(field("replaces"), None);
    assert!(service.get("claim").is_some());
}

#[test]
fn the_stack_carries_the_cores_version() {
    let stack = stack(&[held("curator", CURATOR, Stage::Assembled)]);
    for field in ["stack_version", "min_cli_version"] {
        assert_eq!(
            stack.get(field).and_then(toml::Value::as_str),
            Some("0.18.0")
        );
    }
}

#[test]
fn an_ask_the_bundle_settles_is_written_once_as_the_bundles_row() {
    let stack = stack(&[held("curator", CURATOR, Stage::Assembled)]);
    let rows: Vec<&toml::Value> = stack
        .get("wiring")
        .and_then(toml::Value::as_array)
        .map(|rows| rows.iter().collect())
        .unwrap_or_default();
    let searching: Vec<Option<&str>> = rows
        .iter()
        .filter(|row| row.get("asks").and_then(toml::Value::as_str) == Some("indexer.search"))
        .map(|row| row.get("filled_by").and_then(toml::Value::as_str))
        .collect();
    assert_eq!(searching, vec![Some("finder")]);
    assert!(rows.iter().any(|row| {
        row.get("by").and_then(toml::Value::as_str) == Some("curator")
            && row.get("asks").and_then(toml::Value::as_str) == Some("download.usenet")
    }));
}

#[test]
fn an_installed_part_is_not_assembled() {
    let stack = stack(&[held("curator", CURATOR, Stage::Installed)]);
    assert!(stack.get("service").is_none());
}

#[test]
fn a_part_of_two_services_is_refused_by_name() {
    let two = CURATOR.replace("[[ask]]", "[[service]]\nid = \"second\"\n\n[[ask]]");
    let refused = assembled(BUNDLE, &[held("curator", &two, Stage::Assembled)], "0.18.0");
    assert!(refused.is_err_and(|said| said.iter().any(|one| one.starts_with("curator:"))));
}

#[test]
fn the_assembly_reads_back_as_a_stack_manifest() {
    let Ok(text) = assembled(
        BUNDLE,
        &[held("curator", CURATOR, Stage::Assembled)],
        "0.18.0",
    ) else {
        unreachable!("the fixture assembles");
    };
    assert!(Manifest::from_toml(&text).is_ok_and(|manifest| manifest.services.len() == 1));
}

fn refusal(bundle: &str, copy: &str, pin: impl FnOnce(&mut Pin)) -> Vec<String> {
    let mut part = held("curator", copy, Stage::Assembled);
    pin(&mut part.pin);
    assembled(bundle, &[part], "0.18.0")
        .err()
        .unwrap_or_default()
}

#[test]
fn a_bundle_that_does_not_parse_is_refused() {
    assert_eq!(refusal("= not toml", CURATOR, |_| ()).len(), 1);
}

#[test]
fn each_part_that_cannot_be_a_stack_service_is_refused_saying_why() {
    let unnamed = CURATOR.replace("[[service]]\nid          = \"curator\"\n", "[[service]]\n");
    let said: Vec<(&str, bool)> = [
        ("= not toml", "curator: "),
        ("service = [1]\n", "curator: its service is not a table"),
        (unnamed.as_str(), "curator: its service names no id"),
    ]
    .into_iter()
    .map(|(copy, said)| {
        let refused = refusal(BUNDLE, copy, |_| ());
        (said, refused.iter().any(|one| one.starts_with(said)))
    })
    .collect();
    assert!(said.iter().all(|(_, named)| *named), "{said:?}");
    assert_eq!(
        refusal(BUNDLE, CURATOR, |pin| pin.profile = None),
        vec!["curator: its pin names no profile".to_owned()]
    );
}

#[test]
fn a_word_the_plugin_leaves_out_and_an_ask_naming_no_capability_are_not_written() {
    let sparse = CURATOR
        .replace(
            "without_it  = \"Find and download episodes yourself\"\n",
            "",
        )
        .replace("[[claim]]", "[[ask]]\neach = true\n\n[[claim]]");
    let written = stack(&[held("curator", &sparse, Stage::Assembled)]);
    let whole = stack(&[held("curator", CURATOR, Stage::Assembled)]);
    assert!(only_service(&written).get("without_it").is_none());
    assert!(written.get("wiring").is_some());
    assert_eq!(written.get("wiring"), whole.get("wiring"));
}
