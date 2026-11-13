//! Every instant the web contract publishes is a whole number of seconds since the epoch.
//!
//! Read from the published contract rather than from the code, so what is held is what
//! an SDK decodes: a field described as a moment and typed as text is a moment written
//! some other way, whichever type in the code produced it.
//!
//! [`NOT_INSTANTS`] is what reads like an instant and is not one: a time of day, a
//! calendar date, or a link carrying one in its query. Each entry says why, and an entry the contract no longer holds is
//! refused as well, so the list never outlives what it excuses.

use std::path::Path;

use serde_json::Value;

use crate::source_tree::workspace_root;

/// Text fields described like an instant that are not one, by definition and field.
const NOT_INSTANTS: &[(&str, &str, &str)] = &[
    (
        "Rhythm",
        "from",
        "a time of day on the household's wall clock, HH:MM",
    ),
    (
        "Rhythm",
        "to",
        "a time of day on the household's wall clock, HH:MM",
    ),
    (
        "Title",
        "released",
        "a calendar date, which no time of day belongs to",
    ),
    (
        "Invitation",
        "join",
        "a link, whose expiry is the instant's decimal digits in its query",
    ),
];

/// Whether a description says its field is a moment.
fn described_as_an_instant(description: &str) -> bool {
    let description = description.trim_start();
    let lowered = description.to_lowercase();
    description
        .strip_prefix("When")
        .is_some_and(|rest| !rest.starts_with(char::is_alphanumeric))
        || description.starts_with("Since when")
        || ["since the epoch", "since the unix epoch", "as an instant"]
            .iter()
            .any(|phrase| lowered.contains(phrase))
}

/// Whether a schema's type admits text.
fn is_text(schema: &serde_json::Map<String, Value>) -> bool {
    match schema.get("type") {
        Some(Value::String(kind)) => kind == "string",
        Some(Value::Array(kinds)) => kinds.iter().any(|kind| kind == "string"),
        _ => false,
    }
}

/// Every text field described as an instant under `node`, as `(definition, field)`.
fn walk(node: &Value, definition: &str, field: &str, found: &mut Vec<(String, String)>) {
    match node {
        Value::Object(map) => {
            if let Some(Value::String(description)) = map.get("description") {
                if is_text(map) && described_as_an_instant(description) {
                    found.push((definition.to_owned(), field.to_owned()));
                }
            }
            for (key, value) in map {
                match (key.as_str(), value) {
                    ("$defs" | "definitions", Value::Object(defs)) => {
                        for (name, def) in defs {
                            walk(def, name, "", found);
                        }
                    }
                    ("properties", Value::Object(properties)) => {
                        for (name, property) in properties {
                            walk(property, definition, name, found);
                        }
                    }
                    _ => walk(value, definition, field, found),
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                let named = item.get("name").and_then(Value::as_str).unwrap_or(field);
                walk(item, definition, named, found);
            }
        }
        _ => {}
    }
}

/// Every JSON file beneath `directory`.
fn documents(directory: &Path, into: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            documents(&path, into);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            into.push(path);
        }
    }
}

/// Every text field the web contract describes as an instant, deduplicated.
fn text_instants() -> (usize, Vec<(String, String)>) {
    let mut paths = Vec::new();
    documents(&workspace_root().join("contract/web-api"), &mut paths);
    let mut found = Vec::new();
    for path in &paths {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let Ok(document) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let named = document
            .get("title")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| {
                path.file_stem()
                    .map(|stem| stem.to_string_lossy().into_owned())
            })
            .unwrap_or_default();
        walk(&document, &named, "", &mut found);
    }
    found.sort();
    found.dedup();
    (paths.len(), found)
}

#[test]
fn every_instant_the_web_contract_publishes_is_an_integer() {
    let (read, found) = text_instants();
    assert!(
        read > 0,
        "no contract document was read under contract/web-api"
    );
    let written_as_text: Vec<String> = found
        .iter()
        .filter(|(definition, field)| {
            !NOT_INSTANTS
                .iter()
                .any(|(excused, named, _)| excused == definition && named == field)
        })
        .map(|(definition, field)| format!("{definition}.{field}"))
        .collect();
    assert!(
        written_as_text.is_empty(),
        "each of these is described as an instant and published as text; type it \
         lemonfiber_core::instant::Instant, or name it as the service's own text: {written_as_text:?}"
    );
}

#[test]
fn every_field_excused_as_not_an_instant_is_still_published() {
    let (_, found) = text_instants();
    let stale: Vec<String> = NOT_INSTANTS
        .iter()
        .filter(|(definition, field, _)| {
            !found
                .iter()
                .any(|(held, named)| held == definition && named == field)
        })
        .map(|(definition, field, _)| format!("{definition}.{field}"))
        .collect();
    assert!(
        stale.is_empty(),
        "excused but no longer published as text: {stale:?}"
    );
}
