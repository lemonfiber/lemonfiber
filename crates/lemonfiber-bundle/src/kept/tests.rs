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

/// An installed part's copy: its upstream, taking the place of the stack's service of its
/// id, and its adapter in front of it.
const PLAYER: &str = r#"
[plugin]
id = "player"
description = "Plays what you have"
upstream = "https://example.invalid/player"
license = "MIT"

[[service]]
id = "player"
name = "Player"
image = "example.invalid/player"
native = "player"
replaces = "player"

[service.env]
PLAYER_LOG = { setting = "player:log", default = "info" }
PLAYER_KEY = { secret = "upstream-key" }

[[service]]
id = "player-adapter"
name = "Player adapter"
image = "example.invalid/player-adapter"
fronts = "player"
provides = ["media.serve", "identity.source"]

[[secret]]
id = "upstream-key"
of = "player"
minted = true
why = "w"

[[ask]]
service = "player"
capability = "library.media"

[[ask]]
service = "player-adapter"
capability = "identity.source"
"#;

fn held(id: &str, stage: Stage, copy: &str) -> Held {
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
        copy: copy.to_owned(),
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
    let Ok(text) = kept(ASSEMBLED, held) else {
        unreachable!("the fixture keeps");
    };
    toml::from_str(&text).unwrap_or_default()
}

fn player(stack: &Table) -> Table {
    rows(stack, "service")
        .into_iter()
        .find(|one| field(one, "id") == Some("player"))
        .unwrap_or_default()
}

#[test]
fn an_installed_part_keeps_the_service_its_copy_says_it_takes_the_place_of() {
    let stack = stack(&[
        held("curator", Stage::Assembled, ""),
        held("player", Stage::Installed, PLAYER),
    ]);
    let services = rows(&stack, "service");
    let ids: Vec<Option<&str>> = services.iter().map(|one| field(one, "id")).collect();
    assert_eq!(ids, vec![Some("curator"), Some("player")]);
    let player = player(&stack);
    for (name, value) in [
        ("image", "example.invalid/player"),
        ("profile", "media"),
        ("last_release", "2026-10-01"),
        ("describes", "Plays what you have"),
        ("upstream", "https://example.invalid/player"),
        ("license", "MIT"),
    ] {
        assert_eq!(field(&player, name), Some(value), "{name}");
    }
}

#[test]
fn an_installed_part_s_kept_service_provides_what_the_part_provides() {
    let player = player(&stack(&[held("player", Stage::Installed, PLAYER)]));
    let provided: Vec<&str> = player
        .get("provides")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(provided, vec!["media.serve", "identity.source"]);
}

#[test]
fn what_only_the_plugin_reads_stays_out_of_the_kept_service() {
    let player = player(&stack(&[held("player", Stage::Installed, PLAYER)]));
    assert_eq!(field(&player, "replaces"), None);
    assert_eq!(field(&player, "native"), None);
    let env = player
        .get("env")
        .and_then(Value::as_table)
        .cloned()
        .unwrap_or_default();
    let names: Vec<&String> = env.keys().collect();
    assert_eq!(names, vec!["PLAYER_LOG"]);
}

#[test]
fn an_installed_part_whose_environment_is_only_secrets_keeps_none() {
    let copy = PLAYER.replace(
        "PLAYER_LOG = { setting = \"player:log\", default = \"info\" }\n",
        "",
    );
    let player = player(&stack(&[held("player", Stage::Installed, &copy)]));
    assert!(player.get("env").is_none(), "{player:?}");
}

#[test]
fn an_assembled_part_stays_as_the_bundle_assembled_it() {
    let stack = stack(&[held("curator", Stage::Assembled, "")]);
    let services = rows(&stack, "service");
    assert_eq!(services.len(), 1);
    assert_eq!(
        services.first().and_then(|one| field(one, "name")),
        Some("Curator")
    );
}

#[test]
fn an_installed_part_s_kept_service_asks_nothing_its_copy_asks() {
    let stack = stack(&[
        held("curator", Stage::Assembled, ""),
        held("player", Stage::Installed, PLAYER),
    ]);
    let wiring = rows(&stack, "wiring");
    let asked: Vec<(Option<&str>, Option<&str>)> = wiring
        .iter()
        .map(|row| (field(row, "by"), field(row, "asks")))
        .collect();
    assert_eq!(asked, vec![(Some("curator"), Some("indexer.search"))]);
}

#[test]
fn an_installed_part_whose_copy_declares_no_service_of_its_id_is_refused_by_its_id() {
    let refused = kept(
        ASSEMBLED,
        &[held(
            "absent",
            Stage::Installed,
            "[[service]]\nid = \"elsewhere\"\n",
        )],
    );
    assert_eq!(
        refused,
        Err(vec![
            "absent: its copy declares no service by that id".to_owned()
        ])
    );
    assert!(kept(
        ASSEMBLED,
        &[held("broken", Stage::Installed, "not = [toml")]
    )
    .is_err());
}

#[test]
fn text_that_does_not_parse_is_refused() {
    assert!(kept("not = [toml", &[]).is_err());
}

#[test]
fn a_choice_of_an_installed_parts_adapter_names_the_service_kept_in_its_place() {
    let assembled = format!(
        "{ASSEMBLED}\n[[wiring]]\nby = \"curator\"\nasks = \"media.serve\"\n\
         filled_by = \"player-adapter\"\nwhy = \"w\"\n\n[[wiring]]\nby = \"curator\"\n\
         asks = \"identity.source\"\nfilled_by = \"elsewhere\"\nwhy = \"w\"\n"
    );
    let Ok(text) = kept(&assembled, &[held("player", Stage::Installed, PLAYER)]) else {
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
