//! The OpenAPI document, rendered from the artefact and committed beside it.

use serde_json::{json, Value};

use lemonfiber_api::contract::layout::{self, Files};
use lemonfiber_api::contract::openapi::{rendered, OPENAPI};

use super::fresh;

/// The document this build renders from the artefact it writes.
fn document() -> Value {
    let made = rendered(&fresh());
    assert!(
        made.is_ok(),
        "the OpenAPI document cannot be rendered: {made:?}"
    );
    made.unwrap_or_default()
}

/// Every `$ref` under `node`.
fn references(node: &Value, found: &mut Vec<String>) {
    match node {
        Value::Object(fields) => {
            if let Some(Value::String(target)) = fields.get("$ref") {
                found.push(target.clone());
            }
            fields.values().for_each(|value| references(value, found));
        }
        Value::Array(items) => items.iter().for_each(|item| references(item, found)),
        _ => {}
    }
}

#[test]
fn the_committed_openapi_document_is_what_the_artefact_renders() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let committed = std::fs::read_to_string(root.join(OPENAPI)).unwrap_or_default();
    assert_eq!(
        layout::differing(
            &Files::from([(OPENAPI.to_owned(), committed)]),
            &Files::from([(OPENAPI.to_owned(), layout::rendered(&document()))]),
        ),
        Vec::<String>::new(),
        "run `just contract`"
    );
}

#[test]
fn every_reference_points_inside_the_document() {
    let document = document();
    let mut found = Vec::new();
    references(&document, &mut found);
    let dangling: Vec<&String> = found
        .iter()
        .filter(|target| {
            target
                .strip_prefix("#/components/schemas/")
                .is_none_or(|name| {
                    document
                        .pointer(&format!("/components/schemas/{name}"))
                        .is_none()
                })
        })
        .collect();
    assert!(!found.is_empty());
    assert!(dangling.is_empty(), "{dangling:?}");
}

#[test]
fn it_is_openapi_3_1_with_every_read_action_and_body_as_a_path() {
    let document = document();
    assert_eq!(document.get("openapi"), Some(&json!("3.1.0")));
    assert!(document
        .pointer("/paths/~1api~1actions~1quality-set/post/requestBody")
        .is_some());
    assert!(document
        .pointer("/paths/~1api~1session/post/requestBody")
        .is_some());
    assert_eq!(
        document.pointer("/paths/~1api~1held~1{id}/get/parameters/0"),
        Some(
            &json!({ "name": "id", "in": "path", "required": true, "schema": { "type": "string" } })
        )
    );
    assert_eq!(
        document.pointer("/paths/~1api~1forms/get/parameters/0/schema"),
        Some(&json!({ "type": "array", "items": { "type": "string" } }))
    );
    assert!(document
        .pointer("/paths/~1api~1bundle~1{name}/get/responses/200/content/application~1octet-stream")
        .is_some());
    assert_eq!(
        document
            .pointer("/paths/~1api~1version/get/responses/200/content/application~1json/schema"),
        Some(&json!({ "$ref": "#/components/schemas/kind-version" }))
    );
    assert_eq!(
        document.pointer("/paths/~1api~1events/get/x-lemonfiber-events/kinds/alert/schema"),
        Some(&json!({ "$ref": "#/components/schemas/kind-alert" }))
    );
    assert!(document
        .pointer("/paths/~1api~1jobs~1{job}/delete/responses/202")
        .is_some());
    assert!(!layout::rendered(&document).contains("\"$schema\""));
}

#[test]
fn a_file_the_index_names_that_cannot_be_read_is_refused_by_name() {
    let mut files = fresh();
    files.insert("kinds/version.json".to_owned(), "not json".to_owned());
    let faults = rendered(&files).err().unwrap_or_default();
    assert_eq!(
        faults,
        vec!["kinds/version.json is named by the artefact and cannot be read".to_owned()]
    );
}
