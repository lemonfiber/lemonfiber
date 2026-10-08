//! Seeding reaches whatever fills an ask, and names none of it.
//!
//! A plugin's service standing in for one of the stack's is reached because seeding
//! asks the fillers lookup who answers, and not because somebody remembered to edit a
//! pass. A pass that spelled a bundled service's id, or the port it answers on, would
//! reach that service whatever stood in for it. So nothing under `seed/` spells either,
//! apart from a service the stack itself reaches by name, or one that asks, and each
//! such is listed here with why.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use lemonfiber_manifest::Manifest;

use crate::source_tree::{production, sources, test_only, workspace_root};

/// Where seeding lives.
const SEEDING: &str = "crates/lemonfiber-core/src/seed/";

/// The stack's services filling an ask that seeding spells by id, each with why that is
/// not a consumer naming what fills its ask.
const SPELLED: [(&str, &str); 1] = [(
    "jellyfin",
    "the decline service reaches Jellyfin by name, a link the stack declares with its \
     reason; the identity source is reached through the lookup",
)];

/// The pinned stack, read the way lemonfiber reads it.
fn pinned() -> Option<Manifest> {
    let path = workspace_root().join("assets/media-stack");
    let text = lemonfiber_manifest::read(&path).unwrap_or_default();
    assert!(
        !text.is_empty(),
        "the pinned stack is not at {} — the submodule is not checked out, so this \
         guard would be looking for nothing",
        path.display()
    );
    Manifest::from_toml(&text).ok()
}

/// Every one of the stack's services that fills something an ask asks for, with the
/// ports it answers on.
fn filling(manifest: &Manifest) -> BTreeMap<String, BTreeSet<u16>> {
    let asked: BTreeSet<&str> = manifest
        .wirings
        .iter()
        .filter_map(|wiring| wiring.asks.as_deref())
        .collect();
    manifest
        .services
        .iter()
        .filter(|service| {
            service
                .provides
                .iter()
                .any(|one| asked.contains(one.as_str()))
        })
        .map(|service| {
            let ports = service.listens.into_iter().chain(service.port).collect();
            (service.id.clone(), ports)
        })
        .collect()
}

/// Every service the stack reaches by name, and every service that asks.
fn named_or_asking(manifest: &Manifest) -> BTreeSet<String> {
    manifest
        .wirings
        .iter()
        .flat_map(|wiring| {
            let asking = wiring.asks.as_ref().map(|_| wiring.by.clone());
            asking.into_iter().chain(wiring.to.clone())
        })
        .collect()
}

/// Seeding's shipped files, each as the text it ships with its comments taken out.
fn seeding() -> Vec<(String, String)> {
    sources()
        .into_iter()
        .filter(|(path, _)| {
            let at = path.to_string_lossy().replace('\\', "/");
            at.starts_with(SEEDING) && !test_only(Path::new(&at))
        })
        .map(|(path, text)| {
            let code: Vec<&str> = production(&text)
                .lines()
                .filter(|line| !line.trim_start().starts_with("//"))
                .collect();
            (path.to_string_lossy().replace('\\', "/"), code.join("\n"))
        })
        .collect()
}

/// Whether `text` holds `number` as a whole number rather than inside a longer one.
fn holds_number(text: &str, number: u16) -> bool {
    let wanted = number.to_string();
    text.match_indices(&wanted).any(|(at, _)| {
        let before = text.get(..at).and_then(|head| head.chars().last());
        let after = text
            .get(at + wanted.len()..)
            .and_then(|tail| tail.chars().next());
        !before.is_some_and(|one| one.is_ascii_alphanumeric() || one == '_')
            && !after.is_some_and(|one| one.is_ascii_alphanumeric() || one == '_')
    })
}

/// **Seeding spells no service that fills an ask, and no port one answers on.** Every
/// id of the stack's services filling an ask is looked for in seeding's shipped code as
/// a string, and every port each answers on as a number; only a service the stack
/// reaches by name or one that asks may be spelled, and each such is listed with why.
#[test]
fn seeding_names_nothing_that_fills_an_ask() {
    let manifest = pinned();
    assert!(manifest.is_some(), "the pinned stack parses");
    let fillers = manifest.as_ref().map(filling).unwrap_or_default();
    let excused = manifest.as_ref().map(named_or_asking).unwrap_or_default();
    let files = seeding();
    assert!(
        files.len() > 20 && fillers.len() > 5,
        "the scan read {} files for {} fillers, which means it is looking in the wrong place",
        files.len(),
        fillers.len()
    );

    let mut spelled = BTreeSet::new();
    let mut named = Vec::new();
    for (path, code) in &files {
        for (id, ports) in &fillers {
            if code.contains(&format!("\"{id}\"")) {
                spelled.insert(id.clone());
                if !SPELLED.iter().any(|(listed, _)| listed == id) {
                    named.push(format!("{path}: \"{id}\""));
                }
            }
            for port in ports.iter().filter(|port| holds_number(code, **port)) {
                named.push(format!("{path}: {port}, which {id} answers on"));
            }
        }
    }

    assert!(
        named.is_empty(),
        "seeding names what fills an ask, so a plugin standing in for it would not be \
         reached: {named:?}"
    );
    for (id, why) in SPELLED {
        assert!(
            excused.contains(id),
            "{id} is listed as spelled because {why}, and the stack neither reaches it by \
             name nor has it ask for anything"
        );
        assert!(
            spelled.contains(id),
            "{id} is listed as spelled and seeding no longer spells it, so the list says \
             more than is true"
        );
    }
}

/// The whole-number reading the port check rests on: a port inside a longer number, or
/// inside a name, is not that port.
#[test]
fn a_port_is_read_as_a_whole_number() {
    assert!(holds_number("Some(8989)", 8989));
    assert!(holds_number("8989", 8989));
    assert!(!holds_number("18989", 8989));
    assert!(!holds_number("89890", 8989));
    assert!(!holds_number("v8989", 8989));
    assert!(!holds_number("8989_u16x", 8989));
}
