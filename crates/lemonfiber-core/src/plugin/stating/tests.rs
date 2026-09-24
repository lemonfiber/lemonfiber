use std::path::Path;

use lemonfiber_plugin::Manifest;

use super::{changes, overrides, proofs, Changing, Overriding, Proving, Puts};
use crate::plugin::installed::Installed;

/// A manifest declaring one service, one proof and one override.
const MANIFEST: &str = r#"
schema_version = 1

[plugin]
id          = "komga"
name        = "Komga"
version     = "1.2.0"
description = "Reads your comics on any browser"
without_it  = "Files on disk, no way to read them"
upstream    = "https://example.invalid"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "komga"
name        = "Komga"
image       = "example.invalid/komga"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.11.0"
port        = 25600
bind        = "lan"
criticality = "important"
takes_data  = true
config_path = "/app/data"

[[proof]]
id      = "answers"
title   = "the library API answers"
why     = "a plugin whose service does not answer is not installed"
fixture = "fixtures/libraries.json"
request = { method = "GET", path = "/api/v1/libraries" }
expect  = { status = 200 }

[[override]]
id  = "seerr.settings"
why = "a request for a comic has to reach the library that holds comics"
"#;

/// The stack these are written beneath.
fn stack() -> &'static Path {
    Path::new("/opt/lemonfiber/stack")
}

/// The fixture, with whatever departure the case under test needs made to it
/// first.
///
/// Nothing where the fixture stopped being a manifest, so a fixture that broke
/// fails the assertion it was written for rather than somewhere further down.
fn manifest(change: impl FnOnce(&mut Manifest)) -> Option<Manifest> {
    let mut read = Manifest::from_toml(MANIFEST).ok()?;
    change(&mut read);
    Some(read)
}

/// The fixture as its author wrote it.
fn whole() -> Option<Manifest> {
    manifest(|_| ())
}

/// What installing the fixture writes, beneath the stack above.
fn planned() -> Vec<crate::plugin::Write> {
    whole()
        .map(|read| crate::plugin::writes(&Installed::of(&read), stack()))
        .unwrap_or_default()
}

/// What a manifest states it would change.
fn stated(read: Option<&Manifest>) -> Vec<Proving> {
    read.map(proofs).unwrap_or_default()
}

#[test]
fn every_write_is_stated_as_a_path_and_what_goes_at_it() {
    assert_eq!(
        changes(&planned()),
        vec![
            Changing {
                path: "/opt/lemonfiber/stack/config/komga".to_owned(),
                puts: Puts::Directory,
            },
            Changing {
                path: "/opt/lemonfiber/stack/compose/plugins/komga.yml".to_owned(),
                puts: Puts::Document,
            },
            Changing {
                path: "/opt/lemonfiber/stack/config/caddy/Caddyfile".to_owned(),
                puts: Puts::Region,
            },
            Changing {
                path: "/opt/lemonfiber/stack/config/homepage/services.yaml".to_owned(),
                puts: Puts::Region,
            },
        ]
    );
}

/// Stated in the order the install makes them, which is the order a reversal
/// walks backwards — so what an operator reads and what a reversal would do are
/// one list read in opposite directions.
#[test]
fn the_changes_are_stated_in_the_order_the_install_makes_them() {
    let planned = planned();
    assert_eq!(
        changes(&planned)
            .iter()
            .map(|one| one.path.clone())
            .collect::<Vec<String>>(),
        planned
            .iter()
            .map(|write| write.path.display().to_string())
            .collect::<Vec<String>>()
    );
}

#[test]
fn a_proof_is_stated_with_what_it_asks_of_which_service_and_why() {
    assert_eq!(
        stated(whole().as_ref()),
        vec![Proving {
            proof: "answers".to_owned(),
            establishes: "the library API answers".to_owned(),
            of: Some("komga".to_owned()),
            asks: "GET /api/v1/libraries".to_owned(),
            why: "a plugin whose service does not answer is not installed".to_owned(),
            came_to: None,
        }]
    );
}

/// A plugin that proves nothing and changes nothing of the stack's states
/// neither, which is what lets a surface say so rather than print an empty
/// heading.
#[test]
fn a_plugin_that_proves_nothing_and_overrides_nothing_states_neither() {
    let bare = manifest(|read| {
        read.proofs.clear();
        read.overrides.clear();
    });
    assert!(stated(bare.as_ref()).is_empty());
    assert!(bare.as_ref().map(overrides).unwrap_or_default().is_empty());
}

#[test]
fn a_declared_override_is_stated_with_what_changing_it_is_for() {
    assert_eq!(
        whole().as_ref().map(overrides).unwrap_or_default(),
        vec![Overriding {
            setting: "seerr.settings".to_owned(),
            why: "a request for a comic has to reach the library that holds comics".to_owned(),
        }]
    );
}

/// A proof naming a service the manifest does not declare settles nothing, and
/// says so rather than naming whichever service came first. The reader refuses
/// such a manifest, so this is what a report built from one nobody held to it
/// says.
#[test]
fn a_proof_that_does_not_settle_which_service_it_asks_names_none() {
    let elsewhere = manifest(|read| {
        for proof in &mut read.proofs {
            proof.service = Some("nowhere".to_owned());
        }
    });
    assert_eq!(
        stated(elsewhere.as_ref())
            .into_iter()
            .next()
            .and_then(|one| one.of),
        None
    );
}
