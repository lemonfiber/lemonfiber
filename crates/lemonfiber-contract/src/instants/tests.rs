use lemonfiber_ports::Instant;
use serde_json::{json, Value};

use super::{definition, held};

/// `schema` with the instant defined beside it, as schemars defines it.
fn defining(schema: Value) -> Value {
    let Value::Object(mut whole) = schema else {
        return schema;
    };
    let mut defined = whole
        .remove("$defs")
        .and_then(|defs| defs.as_object().cloned())
        .unwrap_or_default();
    defined.insert("Instant".to_owned(), Value::Object(definition()));
    whole.insert("$defs".to_owned(), Value::Object(defined));
    Value::Object(whole)
}

/// A schema as schemars writes one, its instants referring to the one definition.
fn seen() -> Value {
    defining(json!({
        "type": "object",
        "properties": {
            "at": {"$ref": "#/$defs/Instant"},
            "since": {"anyOf": [{"$ref": "#/$defs/Instant"}, {"type": "null"}]},
            "count": {"type": "integer", "format": "uint64", "minimum": 0},
            "every": {"type": "array", "items": {"$ref": "#/$defs/Instant"}},
            "by_name": {"type": "object", "additionalProperties": {"$ref": "#/$defs/Instant"}},
            "inner": {"anyOf": [{"$ref": "#/$defs/Seen"}, {"type": "null"}]},
        },
        "required": ["at", "count", "every", "by_name"],
        "$defs": {
            "Seen": {"$ref": "#"},
        },
    }))
}

/// A tagged enum's schema as schemars writes one: a variant holding an instant, and one
/// holding a count under the same name.
fn told() -> Value {
    defining(json!({
        "oneOf": [
            {"type": "object", "properties": {"kind": {"type": "string", "const": "added"}, "at": {"$ref": "#/$defs/Instant"}}, "required": ["kind", "at"]},
            {"type": "object", "properties": {"kind": {"type": "string", "const": "counted"}, "at": {"type": "integer"}}, "required": ["kind", "at"]},
        ],
    }))
}

/// A list of pairs, an instant and a count.
fn paired() -> Value {
    defining(json!({
        "type": "array",
        "items": {"type": "array", "prefixItems": [{"$ref": "#/$defs/Instant"}, {"type": "integer"}]},
    }))
}

#[test]
fn every_place_typed_an_instant_is_found_and_no_other_number() {
    let answer = json!({
        "at": 1,
        "since": 2,
        "count": 3,
        "every": [4, 5],
        "by_name": {"a/b": 6, "c~d": 7},
        "inner": {"at": 8, "since": null, "count": 9, "every": [], "by_name": {}, "inner": null},
    });
    assert_eq!(
        held(&seen(), &answer),
        [
            "/at",
            "/by_name/a~1b",
            "/by_name/c~0d",
            "/every/0",
            "/every/1",
            "/inner/at",
            "/since",
        ]
    );
}

#[test]
fn a_place_typed_an_instant_holding_no_unsigned_integer_is_not_one() {
    let answer = json!({"at": "2026-10-26T19:22:31Z", "since": -1, "count": 3, "every": [1.5]});
    assert!(held(&seen(), &answer).is_empty());
}

#[test]
fn a_variant_is_walked_only_where_its_tag_is_the_answers() {
    let told = told();
    assert_eq!(held(&told, &json!({"kind": "added", "at": 1})), ["/at"]);
    assert!(held(&told, &json!({"kind": "counted", "at": 1})).is_empty());
    assert!(held(&told, &json!({"at": 1})).is_empty());
}

#[test]
fn a_list_of_answers_and_a_tuple_are_walked_by_position() {
    assert_eq!(held(&paired(), &json!([[1, 2], [3, 4]])), ["/0/0", "/1/0"]);
    let whole = schemars::schema_for!(Instant).to_value();
    assert_eq!(held(&whole, &json!(1)), [""]);
    assert!(held(&schemars::schema_for!(u64).to_value(), &json!(1)).is_empty());
}

#[test]
fn a_schema_whose_references_lead_nowhere_or_round_finds_nothing() {
    let nowhere = json!({"$ref": "#/$defs/Missing"});
    assert!(held(&nowhere, &json!(1)).is_empty());
    let outside = defining(json!({"$ref": "other.json#/$defs/Instant"}));
    assert!(held(&outside, &json!(1)).is_empty());
    let round = json!({"$ref": "#/$defs/Round", "$defs": {"Round": {"$ref": "#/$defs/Round"}}});
    assert!(held(&round, &json!(1)).is_empty());
    let branching = json!({"anyOf": [{"$ref": "#/$defs/Round"}], "$defs": {"Round": {"$ref": "#/$defs/Round"}}});
    assert!(held(&branching, &json!(1)).is_empty());
    assert!(held(&json!(true), &json!(1)).is_empty());
}

#[test]
fn a_branch_is_ruled_out_by_its_type_constant_list_or_required_member() {
    let at = |said: Value| {
        let one = json!({"properties": {"at": {"$ref": "#/$defs/Instant"}}});
        let mut branch = one.as_object().cloned().unwrap_or_default();
        branch.extend(said.as_object().cloned().unwrap_or_default());
        defining(json!({"anyOf": [branch]}))
    };
    let answer = json!({"at": 1});
    assert_eq!(
        held(&at(json!({"type": ["object", "null"]})), &answer),
        ["/at"]
    );
    assert!(held(&at(json!({"type": "array"})), &answer).is_empty());
    assert!(held(&at(json!({"type": 7})), &answer).is_empty());
    assert!(held(&at(json!({"const": {"at": 2}})), &answer).is_empty());
    assert_eq!(held(&at(json!({"enum": [{"at": 1}]})), &answer), ["/at"]);
    assert!(held(&at(json!({"enum": [{"at": 2}]})), &answer).is_empty());
    assert!(held(&at(json!({"required": ["at", "by"]})), &answer).is_empty());
    assert!(held(&defining(json!({"anyOf": ["no schema"]})), &answer).is_empty());
    let instant =
        defining(json!({"anyOf": [{"$ref": "#/$defs/Instant"}, {"$ref": "#/$defs/Instant"}]}));
    assert_eq!(held(&instant, &json!(1)), [""]);
    assert!(held(&instant, &json!("1")).is_empty());
}

#[test]
fn every_type_a_branch_names_is_told_apart() {
    let with = |name: &str| defining(json!({"anyOf": [{"type": name, "$ref": "#/$defs/Instant"}]}));
    for (name, value) in [
        ("null", json!(null)),
        ("boolean", json!(true)),
        ("number", json!(1.5)),
        ("string", json!("s")),
        ("array", json!([])),
        ("object", json!({})),
    ] {
        assert!(held(&with(name), &value).is_empty(), "{name}");
    }
    assert_eq!(held(&with("integer"), &json!(1)), [""]);
    assert_eq!(held(&with("number"), &json!(1)), [""]);
    assert!(held(&with("integer"), &json!(1.5)).is_empty());
}

#[test]
fn what_a_household_answers_holds_each_members_last_sighting_as_an_instant() {
    let operation = crate::capabilities::all()
        .into_iter()
        .flat_map(|capability| capability.operations)
        .find(|operation| {
            operation.capability == "identity.source" && operation.name == "household"
        });
    let answer = json!([{
        "id": "1",
        "name": "admin",
        "claimed": true,
        "last_seen": 1_790_000_000,
        "access": {"administrator": true, "disabled": false, "every_library": true, "libraries": [], "unrated": "let-through", "age_limit": null},
    }]);
    assert_eq!(
        operation.map(|operation| held(operation.answered.as_value(), &answer)),
        Some(vec!["/0/last_seen".to_owned()])
    );
}
