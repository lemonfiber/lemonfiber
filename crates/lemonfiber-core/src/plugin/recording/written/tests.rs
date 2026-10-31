use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::checked;
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
    Recording {
        recorded_from: "docker.io/plexinc/pms-docker@sha256:e0ab".to_owned(),
        note: "A fresh server.".to_owned(),
        of: Of::Request {
            request: Asked {
                method: "GET".to_owned(),
                path: "/identity".to_owned(),
                accept: Some("application/json".to_owned()),
            },
            response: Answer {
                status: 200,
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
        &varies,
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
