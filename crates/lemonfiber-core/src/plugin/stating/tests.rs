use std::path::Path;

use lemonfiber_plugin::{Manifest, Shape};

use super::{
    asked, changes, overrides, proofs, taking, Changing, Overriding, Proving, Puts, Taking,
};
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

/// The key an adapter service is asked under is stated by where it lands and that it
/// is a key, so a rehearsal names the file without there being a value to show.
#[test]
fn an_adapter_service_states_its_key_by_path() {
    let speaking = manifest(|read| {
        for service in &mut read.services {
            service.speaks = vec!["library.curate@1".to_owned()];
            service.listens = Some(25600);
        }
    });
    let planned = speaking
        .map(|read| crate::plugin::writes(&Installed::of(&read), stack()))
        .unwrap_or_default();
    assert_eq!(
        changes(&planned)
            .into_iter()
            .filter(|one| one.puts == Puts::Key)
            .collect::<Vec<Changing>>(),
        vec![Changing {
            path: "/opt/lemonfiber/stack/config/komga/lemonfiber.key".to_owned(),
            puts: Puts::Key,
        }]
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

#[test]
fn a_service_taking_the_egress_guard_shape_is_stated_with_what_it_is_given() {
    let shaped = manifest(|read| {
        for service in &mut read.services {
            service.shape = Some(Shape::EgressGuard);
        }
    })
    .map(|read| Installed::of(&read));
    assert_eq!(
        shaped.as_ref().map(taking),
        Some(vec![Taking {
            service: "komga".to_owned(),
            shape: Shape::EgressGuard,
            grants: vec!["NET_ADMIN".to_owned()],
            devices: vec!["/dev/net/tun".to_owned()],
            approval: "egress-guard@komga".to_owned(),
        }])
    );
    assert_eq!(
        whole().map(|read| taking(&Installed::of(&read))),
        Some(Vec::new())
    );
}

#[test]
fn a_shape_is_asked_to_be_approved_after_every_value_the_recipes_send_elsewhere() {
    let mut shaped = crate::test_support::an_installed(
        "gluetun",
        vec![crate::plugin::Placed {
            shape: Some(Shape::EgressGuard),
            ..crate::test_support::a_placed("gluetun", &[], None, None)
        }],
    );
    shaped.recipes = vec![crate::plugin::Recipe {
        id: "adopt".to_owned(),
        title: "Adopt".to_owned(),
        why: "Held".to_owned(),
        steps: Vec::new(),
        pairs: vec![crate::plugin::Pair {
            value: "token".to_owned(),
            origin: "operator".to_owned(),
            to: "meta.example.org".to_owned(),
            approval: Some(crate::plugin::approval("token", "meta.example.org")),
            release: None,
            from: None,
        }],
    }];
    let taken = taking(&shaped);
    assert_eq!(
        asked(&shaped, &taken),
        ["token@meta.example.org", "egress-guard@gluetun"]
    );
}
