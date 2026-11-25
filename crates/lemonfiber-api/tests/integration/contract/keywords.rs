//! The artefact uses only the keywords [`keywords`] lists.

use serde_json::Value;

use lemonfiber_api::contract::stability::DESCENDED;
use lemonfiber_api::contract::{keywords, Contract};

/// The keywords whose value names schemas by a key of their own.
const NAMED: [&str; 2] = ["$defs", "properties"];

fn schema_keyword(key: &str, bodies: bool) -> bool {
    keywords::KINDS.contains(&key) || (bodies && keywords::BODIES.contains(&key))
}

fn nested(fields: &serde_json::Map<String, Value>) -> Vec<(String, &Value)> {
    let mut found = Vec::new();
    for (key, value) in fields {
        if NAMED.contains(&key.as_str()) {
            for (name, schema) in value.as_object().into_iter().flatten() {
                found.push((format!("{key}/{name}"), schema));
            }
        } else if DESCENDED.contains(&key.as_str()) {
            match value {
                Value::Array(schemas) => {
                    for (at, schema) in schemas.iter().enumerate() {
                        found.push((format!("{key}/{at}"), schema));
                    }
                }
                schema => found.push((key.clone(), schema)),
            }
        }
    }
    found
}

fn unread_in_schema(node: &Value, path: &str, bodies: bool, found: &mut Vec<String>) {
    let Value::Object(fields) = node else {
        return;
    };
    for key in fields.keys().filter(|key| !schema_keyword(key, bodies)) {
        found.push(format!("{path} uses {key}"));
    }
    for (key, schema) in nested(fields) {
        unread_in_schema(schema, &format!("{path}/{key}"), bodies, found);
    }
}

fn unread(contract: &Value) -> Vec<String> {
    let mut found = Vec::new();
    for (part, bodies) in [("kinds", false), ("bodies", true)] {
        for (name, schema) in contract
            .get(part)
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            unread_in_schema(schema, &format!("{part}/{name}"), bodies, &mut found);
        }
    }
    found.sort();
    found
}

/// The sweep reports each keyword a generator does not read, wherever it is nested.
#[test]
fn a_keyword_no_generator_reads_is_reported() {
    let contract = serde_json::json!({
        "kinds": { "copying": { "$defs": { "Running": {
            "type": "object",
            "properties": { "spans": { "prefixItems": [{ "type": "string" }] } },
            "allOf": [{ "oneOf": [{ "not": {} }] }]
        } } } },
        "bodies": { "answer": { "prefixItems": [{ "if": {} }] } }
    });

    assert_eq!(
        unread(&contract),
        [
            "bodies/answer/prefixItems/0 uses if",
            "kinds/copying/$defs/Running uses allOf",
            "kinds/copying/$defs/Running/allOf/0/oneOf/0 uses not",
            "kinds/copying/$defs/Running/properties/spans uses prefixItems",
        ]
    );
}

/// What every generator reads is not reported.
#[test]
fn what_every_generator_reads_is_not_reported() {
    let contract = serde_json::json!({
        "kinds": { "backup": { "type": "object", "additionalProperties": false, "properties": {
            "scope": { "oneOf": [{ "const": "whole_stack" }], "description": "What it copies." }
        } } },
        "bodies": { "answer": { "prefixItems": [{ "type": "string" }] } }
    });

    assert_eq!(unread(&contract), Vec::<String>::new());
}

/// Fields beside a `oneOf` are read: every generator joins them to each branch, as a struct
/// with one tagged enum flattened into it describes itself.
#[test]
fn fields_beside_a_union_are_read() {
    let contract = serde_json::json!({
        "kinds": { "copying": { "$defs": { "RunningCopy": {
            "type": "object",
            "properties": { "part": { "type": "string" } },
            "required": ["part"],
            "oneOf": [
                { "type": "object", "properties": { "state": { "const": "working" } }, "required": ["state"] },
                { "type": "object", "properties": { "state": { "const": "paced" }, "limit": { "type": "integer" } } }
            ]
        } } } }
    });

    assert_eq!(unread(&contract), Vec::<String>::new());
}

/// The artefact uses no keyword an SDK generator does not read.
#[test]
fn the_artefact_uses_only_what_every_generator_reads() {
    let contract = serde_json::to_value(Contract::describe()).unwrap_or_default();

    assert_eq!(unread(&contract), Vec::<String>::new());
}
