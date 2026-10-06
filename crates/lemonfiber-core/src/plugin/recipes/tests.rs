use lemonfiber_manifest::ApiKind;
use lemonfiber_plugin::Manifest;

use super::{approvals, declared, named, reaching, Adapter, Named, Owner, Pair, Step};

/// A plugin with its own service behind one of lemonfiber's adapters, and one recipe
/// that calls that service, a service of the stack's, and a host outside both — and
/// carries a value it captured to that host, and again to a second one.
const RECIPED: &str = r#"
schema_version = 1

[plugin]
id          = "komga"
name        = "Komga"
version     = "1.2.0"
description = "Reads your comics on any browser"
without_it  = "Files on disk, no way to read them"
upstream    = "https://github.com/gotson/komga"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "komga"
name        = "Komga"
image       = "docker.io/gotson/komga"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.11.0"
port        = 25600
bind        = "lan"
criticality = "important"
listens     = 25600
api         = { kind = "seerr", key_source = "generated" }

[[recipe]]
id    = "adopt"
title = "Hand the library over"
why   = "So the comics are read where the household already looks"

[[recipe.step]]
id      = "in"
call    = { method = "POST", to = "komga", path = "/api/v1/login" }
capture = [{ name = "token", from = "$.token", origin = "komga" }]

[[recipe.step]]
id   = "ask"
call = { method = "GET", to = "sonarr", path = "/api/v3/series" }

[[recipe.step]]
id   = "tell"
call = { method = "POST", to = "metadata.example.org", path = "/v1/series" }

[[recipe.pair]]
value = "token"
to    = "metadata.example.org"

[[recipe.pair]]
value = "token"
to    = "mirror.example.org"

[[recipe.pair]]
value = "token"
to    = "metadata.example.org"
"#;

/// The manifest above, read the way an install reads one.
fn manifest() -> Option<Manifest> {
    Manifest::from_toml(RECIPED).ok()
}

/// The stack this build pins.
const STACK: &str = include_str!("../../../../../assets/media-stack/stack.toml");

/// The stack this build pins, read the way a run reads it.
fn stack() -> Option<lemonfiber_manifest::Manifest> {
    lemonfiber_manifest::Manifest::from_toml(STACK).ok()
}

#[test]
fn every_call_is_kept_in_order_with_its_own_services_adapter() {
    let recipes = manifest().map(|manifest| declared(&manifest));
    let steps: Option<Vec<Step>> = recipes
        .as_ref()
        .and_then(|recipes| recipes.first())
        .map(|recipe| recipe.steps.clone());
    assert_eq!(
        steps.as_ref().map(|steps| steps
            .iter()
            .map(|step| step.id.as_str())
            .collect::<Vec<_>>()),
        Some(vec!["in", "ask", "tell"])
    );
    assert_eq!(
        steps
            .as_ref()
            .and_then(|steps| steps.first())
            .map(|step| step.adapter),
        Some(Some(Adapter {
            kind: ApiKind::Seerr,
            owner: Owner::Lemonfiber,
        })),
        "a call to the plugin's own service names that service's adapter"
    );
    assert_eq!(
        steps.as_ref().map(|steps| steps
            .iter()
            .skip(1)
            .map(|step| step.adapter)
            .collect::<Vec<_>>()),
        Some(vec![None, None]),
        "with no stack read, nothing else has one yet"
    );
}

#[test]
fn a_call_to_the_stacks_own_service_names_its_adapter_once_the_stack_is_read() {
    let reached = manifest()
        .zip(stack())
        .map(|(manifest, stack)| reaching(declared(&manifest), &stack));
    let adapters: Option<Vec<Option<Adapter>>> = reached
        .as_ref()
        .and_then(|recipes| recipes.first())
        .map(|recipe| recipe.steps.iter().map(|step| step.adapter).collect());
    assert_eq!(
        adapters,
        Some(vec![
            Some(Adapter {
                kind: ApiKind::Seerr,
                owner: Owner::Lemonfiber
            }),
            Some(Adapter {
                kind: ApiKind::Servarr,
                owner: Owner::Lemonfiber
            }),
            None,
        ]),
        "sonarr is reached through lemonfiber's servarr adapter, and a host outside through none"
    );
}

#[test]
fn every_pair_carries_its_origin_and_what_approving_it_is_written_as() {
    let pairs: Option<Vec<Pair>> = manifest()
        .map(|manifest| declared(&manifest))
        .and_then(|recipes| recipes.first().map(|recipe| recipe.pairs.clone()));
    assert_eq!(
        pairs.as_ref().and_then(|pairs| pairs.first()).cloned(),
        Some(Pair {
            value: "token".to_owned(),
            origin: "komga".to_owned(),
            to: "metadata.example.org".to_owned(),
            approval: "token@metadata.example.org".to_owned(),
        })
    );
}

#[test]
fn each_approval_is_asked_for_once_in_the_order_declared() {
    let recipes = manifest()
        .map(|manifest| declared(&manifest))
        .unwrap_or_default();
    assert_eq!(
        approvals(&recipes),
        vec!["token@metadata.example.org", "token@mirror.example.org"]
    );
}

#[test]
fn a_value_no_step_captures_has_no_origin_rather_than_an_invented_one() {
    let text = RECIPED.replace(
        "capture = [{ name = \"token\", from = \"$.token\", origin = \"komga\" }]",
        "",
    );
    let pairs = Manifest::from_toml(&text)
        .ok()
        .map(|manifest| declared(&manifest))
        .and_then(|recipes| recipes.first().map(|recipe| recipe.pairs.clone()));
    assert_eq!(
        pairs.and_then(|pairs| pairs.first().map(|pair| pair.origin.clone())),
        Some(String::new())
    );
}

#[test]
fn every_adapter_the_plugins_services_name_is_said_to_be_lemonfibers() {
    assert_eq!(
        manifest().map(|manifest| named(&manifest)),
        Some(vec![Named {
            service: "komga".to_owned(),
            kind: ApiKind::Seerr,
            owner: Owner::Lemonfiber,
        }])
    );
}

#[test]
fn an_adapter_says_whose_it_is_on_the_wire() {
    let said = serde_json::to_string(&Adapter {
        kind: ApiKind::Servarr,
        owner: Owner::Lemonfiber,
    })
    .unwrap_or_default();
    assert_eq!(said, r#"{"kind":"servarr","owner":"lemonfiber"}"#);
}
