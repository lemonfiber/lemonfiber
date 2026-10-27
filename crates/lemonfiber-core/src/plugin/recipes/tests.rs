use lemonfiber_manifest::ApiKind;
use lemonfiber_plugin::Manifest;

use super::{approvals, declared, reaching, Adapter, Owner, Pair, Step};

/// A plugin with its own service, and one recipe that calls that service, a service of
/// the stack's, and a host outside both — and carries a value it captured to that host,
/// and again to a second one.
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

[[recipe]]
id    = "adopt"
title = "Hand the library over"
why   = "So the comics are read where the household already looks"

[[recipe.step]]
id      = "in"
call    = { method = "POST", to = "komga", path = "/api/v1/login" }
capture = [{ name = "token", from = "token", origin = "stack-service" }]

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

[[recipe.pair]]
value = "token"
to    = "komga"
"#;

/// The manifest above, read the way an install reads one.
fn manifest() -> Option<Manifest> {
    Manifest::from_toml(RECIPED).ok()
}

#[test]
fn every_call_is_kept_in_order_with_no_adapter_until_the_stack_is_read() {
    let recipes = manifest().map(|manifest| declared(&manifest));
    let steps: Option<Vec<Step>> = recipes
        .as_ref()
        .and_then(|recipes| recipes.first())
        .map(|recipe| recipe.steps.clone());
    assert_eq!(
        steps.as_ref().map(|steps| steps
            .iter()
            .map(|step| (step.id.as_str(), step.adapter))
            .collect::<Vec<_>>()),
        Some(vec![("in", None), ("ask", None), ("tell", None)])
    );
}

/// The adapter each step of the first recipe names once read beside `installed`.
fn adapters_against(installed: &[crate::plugin::Installed]) -> Option<Vec<Option<Adapter>>> {
    manifest()
        .map(|manifest| reaching(declared(&manifest), installed))
        .as_ref()
        .and_then(|recipes| recipes.first())
        .map(|recipe| recipe.steps.iter().map(|step| step.adapter).collect())
}

/// What the steps name where sonarr is reached in its servarr API.
fn through_servarr() -> Vec<Option<Adapter>> {
    vec![
        None,
        Some(Adapter {
            kind: ApiKind::Servarr,
            owner: Owner::Lemonfiber,
        }),
        None,
    ]
}

#[test]
fn a_call_to_a_moved_part_names_the_api_the_service_moved_in_its_place_answers_in() {
    let installed: Vec<_> = crate::test_support::curators::TELEVISION
        .installed()
        .into_iter()
        .collect();
    assert_eq!(
        adapters_against(&installed),
        Some(through_servarr()),
        "sonarr is reached in the servarr API, and a host outside in none"
    );
    assert_eq!(
        adapters_against(&[]),
        Some(vec![None, None, None]),
        "with nothing moved in its place, the shipped sonarr names no API"
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
            origin: "stack-service".to_owned(),
            to: "metadata.example.org".to_owned(),
            approval: Some("token@metadata.example.org".to_owned()),
            release: None,
            from: None,
        })
    );
}

/// A pair releasing a capture names its release and the service the value was read
/// from, and asks for its approval though it never leaves the stack.
#[test]
fn a_released_pair_says_why_where_from_and_what_approving_it_is_written_as() {
    let released = format!(
        "{RECIPED}\n[[recipe.pair]]\nvalue   = \"token\"\nto      = \"sonarr\"\n\
         release = \"Sonarr files what Komga names.\"\n"
    );
    let pair = Manifest::from_toml(&released)
        .ok()
        .map(|manifest| declared(&manifest))
        .and_then(|recipes| {
            recipes
                .first()
                .and_then(|recipe| recipe.pairs.last().cloned())
        });
    assert_eq!(
        pair,
        Some(Pair {
            value: "token".to_owned(),
            origin: "stack-service".to_owned(),
            to: "sonarr".to_owned(),
            approval: Some("token@sonarr".to_owned()),
            release: Some("Sonarr files what Komga names.".to_owned()),
            from: Some("komga".to_owned()),
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
        "capture = [{ name = \"token\", from = \"token\", origin = \"stack-service\" }]",
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

/// A value an input brings in carries the origin the input writes, whichever it is.
#[test]
fn a_value_an_input_brings_in_carries_the_inputs_origin() {
    for (origin, extra) in [
        ("operator", "ask    = \"The code\""),
        ("credential-store", "of     = \"sonarr\""),
    ] {
        let text = RECIPED
            .replace(
                "capture = [{ name = \"token\", from = \"token\", origin = \"stack-service\" }]",
                "",
            )
            .replace(
                "[[recipe.step]]\nid      = \"in\"",
                &format!(
                    "[[recipe.input]]\nname   = \"token\"\norigin = \"{origin}\"\n{extra}\n\n\
                     [[recipe.step]]\nid      = \"in\""
                ),
            );
        let pairs = Manifest::from_toml(&text)
            .ok()
            .map(|manifest| declared(&manifest))
            .and_then(|recipes| recipes.first().map(|recipe| recipe.pairs.clone()));
        assert_eq!(
            pairs.and_then(|pairs| pairs.first().map(|pair| pair.origin.clone())),
            Some(origin.to_owned()),
            "{text}"
        );
    }
}

/// A reading lists every input a recipe asks the operator for, with its question and
/// whether it is a secret, and none the credential store answers.
#[test]
fn a_recipe_lists_what_it_asks_the_operator_for_and_nothing_the_store_answers() {
    let text = RECIPED
        .replace(
            "[[recipe.step]]\nid      = \"in\"",
            "on = \"demand\"\n\n\
             [[recipe.input]]\nname   = \"code\"\norigin = \"operator\"\nask    = \"The claim code\"\nsecret = true\n\n\
             [[recipe.input]]\nname   = \"user\"\norigin = \"operator\"\n\n\
             [[recipe.input]]\nname   = \"key\"\norigin = \"credential-store\"\nof     = \"sonarr\"\n\n\
             [[recipe.step]]\nid      = \"in\"",
        );
    let read = Manifest::from_toml(&text);
    assert!(read.is_ok(), "{read:?}");
    let recipe = read
        .ok()
        .map(|manifest| declared(&manifest))
        .and_then(|recipes| recipes.into_iter().next());

    assert_eq!(
        recipe.as_ref().map(|one| one.on),
        Some(lemonfiber_plugin::On::Demand)
    );
    assert_eq!(
        recipe.map(|one| one.inputs),
        Some(vec![
            super::RecipeInput {
                name: "code".to_owned(),
                ask: "The claim code".to_owned(),
                secret: true,
            },
            super::RecipeInput {
                name: "user".to_owned(),
                ask: "user".to_owned(),
                secret: false,
            },
        ])
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

/// A pair to a service in this stack takes nothing off the machine and asks for no
/// approval; one to a host outside asks for its own.
#[test]
fn only_a_pair_to_a_host_outside_asks_for_approval() {
    let pairs: Vec<Pair> = manifest()
        .map(|manifest| declared(&manifest))
        .and_then(|recipes| recipes.first().map(|recipe| recipe.pairs.clone()))
        .unwrap_or_default();
    let inside = pairs.iter().find(|pair| pair.to == "komga");
    assert_eq!(inside.map(|pair| pair.approval.clone()), Some(None));
    assert!(pairs
        .iter()
        .filter(|pair| pair.to != "komga")
        .all(|pair| pair.approval.is_some()));
}
