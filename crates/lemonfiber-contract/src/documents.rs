//! The published documents: an `OpenAPI` document, a conformance list and an index of its
//! files per capability, and an index of every capability.
//!
//! Generated from [`crate::capabilities::all`], never written by hand, and compared with
//! the committed `contract/capabilities/` by a test, so a type that moves and a document
//! that did not is a failed build.

use serde_json::{json, Value};

use crate::{wire, Capability};

/// Where the documents are committed, from the workspace root.
pub const DIRECTORY: &str = "contract/capabilities";

/// The scheme every operation is asked under.
const KEY: &str = "key";

/// Every file the documents are, as a path under [`DIRECTORY`] and its contents.
#[must_use]
pub fn files() -> Vec<(String, String)> {
    let capabilities = crate::capabilities::all();
    let mut files = Vec::new();
    let mut listed = Vec::new();
    for capability in &capabilities {
        let documents = documents(capability);
        let own = written(&index(capability, &documents));
        listed.push(entry(capability, &own));
        files.extend(documents);
        files.push((located(capability, INDEX), own));
    }
    files.push((
        INDEX.to_owned(),
        written(&json!({ "capabilities": listed })),
    ));
    files
}

/// One capability's documents, each as a path under [`DIRECTORY`] and its contents.
fn documents(capability: &Capability) -> Vec<(String, String)> {
    let mut documents = vec![(located(capability, OPENAPI), openapi(capability))];
    documents.extend(capability.operations.iter().map(|operation| {
        (
            located(capability, &format!("{OPERATIONS}/{}.json", operation.name)),
            path_item(operation),
        )
    }));
    documents.push((located(capability, CONFORMANCE), conformance(capability)));
    documents
        .into_iter()
        .map(|(path, document)| (path, written(&document)))
        .collect()
}

/// The name of the index of every capability, and of each capability's own.
const INDEX: &str = "index.json";

/// The name of a capability's `OpenAPI` document.
const OPENAPI: &str = "openapi.json";

/// The directory beside it holding one document per operation, so no generated file
/// grows past what a reader can hold, however many operations a capability carries.
const OPERATIONS: &str = "operations";

/// The name of a capability's conformance list.
const CONFORMANCE: &str = "conformance.json";

/// Where the refusal every operation may answer with is described once.
const REFUSED: &str = "../openapi.json#/components/responses/Refused";

/// Where one of a capability's documents sits, under [`DIRECTORY`].
fn located(capability: &Capability, document: &str) -> String {
    format!("{}/v{}/{document}", capability.name, capability.major)
}

/// SHA-256 over a document as it is committed, lower-case hex.
fn digest(text: &str) -> String {
    use std::fmt::Write as _;
    ring::digest::digest(&ring::digest::SHA256, text.as_bytes())
        .as_ref()
        .iter()
        .fold(String::new(), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}

/// A document as it is committed: pretty, with a closing newline.
fn written(document: &Value) -> String {
    let mut text = serde_json::to_string_pretty(document).unwrap_or_default();
    text.push('\n');
    text
}

/// One capability's `OpenAPI` document: each operation's path pointing at the document
/// that describes it, and what every operation shares described once.
#[must_use]
pub fn openapi(capability: &Capability) -> Value {
    let paths: serde_json::Map<String, Value> = capability
        .operations
        .iter()
        .map(|operation| {
            (
                operation.path(),
                json!({ "$ref": format!("{OPERATIONS}/{}.json", operation.name) }),
            )
        })
        .collect();
    let refusal = serde_json::to_value(schemars::schema_for!(wire::Refusal)).unwrap_or_default();
    json!({
        "openapi": "3.1.0",
        "info": {
            "title": format!("{} contract", capability.name),
            "version": capability.major.to_string()
        },
        "paths": paths,
        "components": {
            "securitySchemes": { KEY: { "type": "http", "scheme": "bearer" } },
            "schemas": { "Refusal": refusal },
            "responses": {
                "Refused": {
                    "description": "A refusal the contract declares.",
                    "content": { wire::PROBLEM: { "schema": { "$ref": "#/components/schemas/Refusal" } } }
                }
            }
        }
    })
}

/// One operation, as the path item its capability's document points at.
#[must_use]
pub fn path_item(operation: &crate::Operation) -> Value {
    let refused = json!({ "$ref": REFUSED });
    let mut item = json!({
        "post": {
            "operationId": operation.name,
            "x-lemonfiber-largest": operation.bounds.largest,
            "x-lemonfiber-deadline": operation.bounds.deadline.as_secs(),
            "security": [{ KEY: [] }],
            "requestBody": {
                "required": true,
                "content": { wire::JSON: { "schema": operation.asked } }
            },
            "responses": {
                "200": {
                    "description": "What the operation answers.",
                    "content": { wire::JSON: { "schema": operation.answered } }
                },
                "204": { "description": "Answered, with nothing to say." },
                "401": { "description": "The plugin's key was absent or wrong." },
                "400": refused,
                "404": refused,
                "500": refused,
                "502": refused,
                "503": refused
            }
        }
    });
    if let (true, Some(responses)) = (
        operation.restarts,
        item.pointer_mut("/post/responses")
            .and_then(Value::as_object_mut),
    ) {
        responses.insert(
            wire::RESET.to_string(),
            json!({ "description": "Applied, and in force once the upstream restarts." }),
        );
    }
    if let (false, Some(post)) = (
        operation.secret.is_empty(),
        item.get_mut("post").and_then(Value::as_object_mut),
    ) {
        post.insert("x-lemonfiber-secret".to_owned(), json!(operation.secret));
    }
    item
}

/// The cases an adapter's recordings and its live pass must answer for one capability,
/// as [`crate::conformance::cases`] lists them.
#[must_use]
pub fn conformance(capability: &Capability) -> Value {
    let cases: Vec<Value> = crate::conformance::cases(capability)
        .into_iter()
        .map(|case| {
            let expect = match case.expect {
                crate::conformance::Expect::Status(statuses) => json!({ "status": statuses }),
                crate::conformance::Expect::Conforms => json!({ "conforms": true }),
            };
            let mut listed = json!({
                "case": case.case,
                "operation": case.operation.name,
                "keyed": case.keyed,
                "expect": expect,
                "live": case.live,
                "asked": case.asks
            });
            if let (Some(media_type), Some(fields)) = (case.media_type, listed.as_object_mut()) {
                fields.insert("media_type".to_owned(), json!(media_type));
            }
            listed
        })
        .collect();
    json!({
        "capability": capability.name,
        "major": capability.major,
        "cases": cases
    })
}

/// One capability's index: every file of its documents with its digest.
#[must_use]
pub fn index(capability: &Capability, documents: &[(String, String)]) -> Value {
    let files: Vec<Value> = documents
        .iter()
        .map(|(path, text)| json!({ "path": path, "sha256": digest(text) }))
        .collect();
    json!({
        "capability": capability.name,
        "major": capability.major,
        "files": files
    })
}

/// One capability as the index of every capability lists it: the major this build
/// speaks, where its documents are, and its own index, `own`, with its digest.
fn entry(capability: &Capability, own: &str) -> Value {
    json!({
        "capability": capability.name,
        "majors": [capability.major],
        "operations": capability.operations.len(),
        "facet": crate::capabilities::is_facet(capability.name),
        "openapi": located(capability, OPENAPI),
        "conformance": located(capability, CONFORMANCE),
        "index": located(capability, INDEX),
        "sha256": digest(own)
    })
}

#[cfg(test)]
mod tests;
