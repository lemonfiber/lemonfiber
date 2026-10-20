use std::collections::BTreeMap;

use toml::{Table, Value};

use super::kept;
use crate::{Held, Pin, Stage};

const ASSEMBLED: &str = r#"
schema_version = 1
stack_version = "0.18.0"
min_cli_version = "0.18.0"

[[wiring]]
by = "curator"
asks = "indexer.search"

[[service]]
id = "curator"
name = "Curator"
"#;

const FROZEN: &str = r#"
schema_version = 1
stack_version = "0.1.0"
min_cli_version = "0.1.0"

[[wiring]]
by = "player"
asks = "library.media"

[[wiring]]
by = "curator"
asks = "indexer.search"
filled_by = "finder"

[[service]]
id = "player"
name = "Player"
image = "example.invalid/player"

[[service]]
id = "curator"
name = "Curator, as the stack had it"
"#;

fn held(id: &str, stage: Stage) -> Held {
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
            profile: Some("media".into()),
            last_release: Some("2026-10-01".into()),
        },
        copy: String::new(),
        tree: BTreeMap::new(),
    }
}

fn rows(stack: &Table, name: &str) -> Vec<Table> {
    stack
        .get(name)
        .and_then(Value::as_array)
        .map(|rows| rows.iter().filter_map(Value::as_table).cloned().collect())
        .unwrap_or_default()
}

fn field<'a>(row: &'a Table, name: &str) -> Option<&'a str> {
    row.get(name).and_then(Value::as_str)
}

fn stack(held: &[Held]) -> Table {
    let Ok(text) = kept(ASSEMBLED, FROZEN, held) else {
        unreachable!("the fixture keeps");
    };
    toml::from_str(&text).unwrap_or_default()
}

#[test]
fn an_installed_part_keeps_the_service_the_frozen_stack_declares() {
    let stack = stack(&[
        held("curator", Stage::Assembled),
        held("player", Stage::Installed),
    ]);
    let services = rows(&stack, "service");
    let ids: Vec<Option<&str>> = services.iter().map(|one| field(one, "id")).collect();
    assert_eq!(ids, vec![Some("curator"), Some("player")]);
    assert_eq!(
        services.get(1).and_then(|one| field(one, "image")),
        Some("example.invalid/player")
    );
}

#[test]
fn an_assembled_part_stays_as_the_bundle_assembled_it() {
    let stack = stack(&[held("curator", Stage::Assembled)]);
    let services = rows(&stack, "service");
    assert_eq!(services.len(), 1);
    assert_eq!(
        services.first().and_then(|one| field(one, "name")),
        Some("Curator")
    );
}

#[test]
fn an_installed_part_keeps_the_rows_it_asks_by_and_no_other() {
    let stack = stack(&[
        held("curator", Stage::Assembled),
        held("player", Stage::Installed),
    ]);
    let wiring = rows(&stack, "wiring");
    let asked: Vec<(Option<&str>, Option<&str>)> = wiring
        .iter()
        .map(|row| (field(row, "by"), field(row, "asks")))
        .collect();
    assert_eq!(
        asked,
        vec![
            (Some("curator"), Some("indexer.search")),
            (Some("player"), Some("library.media")),
        ]
    );
    assert_eq!(wiring.first().and_then(|row| field(row, "filled_by")), None);
}

#[test]
fn an_installed_part_the_frozen_stack_does_not_declare_is_refused_by_its_id() {
    let refused = kept(ASSEMBLED, FROZEN, &[held("absent", Stage::Installed)]);
    assert_eq!(
        refused,
        Err(vec![
            "absent: the stack the bundle replaces declares no service by that id".to_owned()
        ])
    );
}

#[test]
fn text_that_does_not_parse_is_refused() {
    assert!(kept("not = [toml", FROZEN, &[]).is_err());
    assert!(kept(ASSEMBLED, "not = [toml", &[]).is_err());
}

#[test]
fn a_choice_of_an_installed_parts_adapter_names_the_service_kept_in_its_place() {
    let assembled = format!(
        "{ASSEMBLED}\n[[wiring]]\nby = \"curator\"\nasks = \"media.serve\"\n\
         filled_by = \"player-adapter\"\nwhy = \"w\"\n\n[[wiring]]\nby = \"curator\"\n\
         asks = \"identity.source\"\nfilled_by = \"elsewhere\"\nwhy = \"w\"\n"
    );
    let mut player = held("player", Stage::Installed);
    player.copy =
        "[[service]]\nid = \"player\"\n\n[[service]]\nid = \"player-adapter\"\n".to_owned();
    let Ok(text) = kept(&assembled, FROZEN, &[player]) else {
        unreachable!("the fixture keeps");
    };
    let stack: Table = toml::from_str(&text).unwrap_or_default();
    let wiring = rows(&stack, "wiring");
    let chosen: Vec<(Option<&str>, Option<&str>)> = wiring
        .iter()
        .filter(|row| field(row, "filled_by").is_some())
        .map(|row| (field(row, "asks"), field(row, "filled_by")))
        .collect();
    assert_eq!(
        chosen,
        vec![
            (Some("media.serve"), Some("player")),
            (Some("identity.source"), Some("elsewhere")),
        ]
    );
}
