use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::{checked, Varying};
use crate::plugin::recorded::{Answer, Asked, Of, Recording};
use crate::plugin::recording::Took;

/// Where a Plex server says which server it is.
const PLACE: &str = "/MediaContainer/machineIdentifier";

/// The identity a Plex server answered on one run.
const FIRST: &str = "946d83ffdbc9067a9ff34de253bff49bec92708f";

/// The identity it answered on another.
const SECOND: &str = "0b7d4c2a8e6f1b3d5a7c9e0f2b4d6a8c5f3c9a1e";

/// A recording of `/identity` answering `json`.
fn identity(json: Value) -> Recording {
    answered_with(200, json)
}

/// A recording of `/identity` answering `json` with `status`.
fn answered_with(status: u16, json: Value) -> Recording {
    Recording {
        recorded_from: "docker.io/plexinc/pms-docker@sha256:e0ab".to_owned(),
        note: "A fresh server.".to_owned(),
        of: Of::Request {
            request: Asked {
                method: "GET".to_owned(),
                path: "/identity".to_owned(),
                accept: Some("application/json".to_owned()),
                headers: std::collections::BTreeMap::new(),
            },
            response: Answer {
                status,
                json: Some(json),
                ..Answer::default()
            },
        },
    }
}

/// What holding a recording answering `held` to one answering `taken` comes to, with
/// `varies` declared.
fn held_to(held: &Value, taken: Value, varies: &[(&str, &str)]) -> Took {
    let varies: BTreeMap<String, String> = varies
        .iter()
        .map(|(place, shape)| ((*place).to_owned(), (*shape).to_owned()))
        .collect();
    let written = crate::plugin::rendered(&identity(held.clone())).unwrap_or_default();
    checked(
        "identity.json",
        Some(&written),
        &identity(taken),
        false,
        Varying::declared(&varies),
    )
}

/// The answer a Plex server gives with `identifier` at [`PLACE`] and `version` beside it.
fn answering(identifier: &str, version: &str) -> Value {
    json!({ "MediaContainer": { "machineIdentifier": identifier, "version": version } })
}

#[test]
fn a_declared_place_varies_within_its_shape_and_is_taken_again() {
    let took = held_to(
        &answering(FIRST, "1.43.4"),
        answering(SECOND, "1.43.4"),
        &[(PLACE, "[0-9a-f]{40}")],
    );
    assert_eq!(took, Took::Reproduced);
}

#[test]
fn the_same_kind_of_value_at_a_place_nobody_declared_still_has_to_match() {
    let took = held_to(
        &answering(FIRST, FIRST),
        answering(SECOND, SECOND),
        &[(PLACE, "[0-9a-f]{40}")],
    );
    assert!(matches!(took, Took::Unreproduced { .. }), "{took:?}");
    let undeclared = held_to(
        &answering(FIRST, "1.43.4"),
        answering(SECOND, "1.43.4"),
        &[],
    );
    assert!(
        matches!(undeclared, Took::Unreproduced { .. }),
        "{undeclared:?}"
    );
}

#[test]
fn a_value_out_of_its_shape_or_missing_at_a_declared_place_is_not_taken_again() {
    for taken in [
        answering("946d83ff", "1.43.4"),
        answering(&format!("{FIRST}0"), "1.43.4"),
        json!({ "MediaContainer": { "version": "1.43.4" } }),
    ] {
        let took = held_to(
            &answering(FIRST, "1.43.4"),
            taken.clone(),
            &[(PLACE, "[0-9a-f]{40}")],
        );
        assert!(
            matches!(took, Took::Unreproduced { .. }),
            "{taken}: {took:?}"
        );
    }
}

/// What holding a recording answering `held` with `status` to one answering `taken` comes
/// to, where what is answered is a household and each member's last sighting an instant.
fn household_held_to(status: u16, held: &Value, taken: Value) -> Took {
    let household = lemonfiber_contract::capabilities::all()
        .into_iter()
        .flat_map(|capability| capability.operations)
        .find(|operation| {
            operation.capability == "identity.source" && operation.name == "household"
        })
        .map(|operation| operation.answered.as_value().clone())
        .unwrap_or_default();
    let written = crate::plugin::rendered(&answered_with(status, held.clone())).unwrap_or_default();
    checked(
        "household.json",
        Some(&written),
        &answered_with(status, taken),
        false,
        Varying::answered(&household),
    )
}

/// A household of one member, last seen at `last_seen`.
fn seen_at(last_seen: &Value) -> Value {
    json!([{
        "id": "<redacted>",
        "name": "admin",
        "claimed": true,
        "last_seen": last_seen,
        "access": {"administrator": true, "disabled": false, "every_library": true, "libraries": [], "unrated": "let-through", "age_limit": null},
    }])
}

#[test]
fn an_instant_the_answer_types_differs_on_every_run_and_is_taken_again() {
    let took = household_held_to(
        200,
        &seen_at(&json!(1_790_000_000)),
        seen_at(&json!(1_790_000_042)),
    );
    assert_eq!(took, Took::Reproduced);
}

#[test]
fn an_instant_out_of_its_shape_or_in_a_refusal_has_to_match() {
    for taken in [
        seen_at(&json!("1790000042")),
        seen_at(&json!(-1)),
        seen_at(&Value::Null),
    ] {
        let took = household_held_to(200, &seen_at(&json!(1_790_000_000)), taken.clone());
        assert!(
            matches!(took, Took::Unreproduced { .. }),
            "{taken}: {took:?}"
        );
    }
    let refused = household_held_to(
        500,
        &seen_at(&json!(1_790_000_000)),
        seen_at(&json!(1_790_000_042)),
    );
    assert!(matches!(refused, Took::Unreproduced { .. }), "{refused:?}");
}

#[test]
fn a_number_the_answer_does_not_type_an_instant_has_to_match() {
    let took = household_held_to(
        200,
        &json!([{"id": "1", "name": "admin", "claimed": true, "last_seen": null, "count": 1}]),
        json!([{"id": "1", "name": "admin", "claimed": true, "last_seen": null, "count": 2}]),
    );
    assert!(matches!(took, Took::Unreproduced { .. }), "{took:?}");
}
