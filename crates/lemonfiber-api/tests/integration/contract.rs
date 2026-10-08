//! The contract artefact describes exactly what this build writes.
//!
//! Every kind is described, every outcome is described as the document it writes,
//! the committed artefacts match what the types generate, and the stable surface a
//! consumer relies on never loses anything under an unchanged wire version.

mod definitions;
mod reads;
mod refusals;
mod samples;
mod split;
mod surface;

use std::collections::{BTreeSet, HashSet};

use serde_json::Value;

use lemonfiber_api::contract::layout::{self, Files};
use lemonfiber_api::contract::{Contract, CONTRACT_DIR};
use lemonfiber_core::app::Outcome;

use samples::samples;

/// What is committed, read from the workspace root.
fn committed() -> Files {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    layout::read(&root.join(CONTRACT_DIR)).unwrap_or_default()
}

/// The directory this build writes, with whatever kept it from being written.
fn fresh() -> Files {
    let written = Contract::describe().files();
    assert!(
        written.is_ok(),
        "the contract cannot be written: {written:?}"
    );
    written.unwrap_or_default()
}

/// The schema the contract publishes for one kind's `data`, with its `$ref`
/// resolved to the definition it names.
fn payload(kind: &str) -> Value {
    let contract = serde_json::to_value(Contract::describe()).unwrap_or_default();
    let schema = contract
        .pointer(&format!("/kinds/{kind}"))
        .cloned()
        .unwrap_or_default();
    let reference = schema
        .pointer("/properties/data/$ref")
        .and_then(Value::as_str)
        .unwrap_or_default();
    schema
        .pointer(reference.trim_start_matches('#'))
        .cloned()
        .unwrap_or_default()
}

/// The fields a payload schema describes.
fn described_fields(payload: &Value) -> BTreeSet<String> {
    payload
        .get("properties")
        .and_then(Value::as_object)
        .map(|fields| fields.keys().cloned().collect())
        .unwrap_or_default()
}

/// The fields a payload schema insists on.
fn required_fields(payload: &Value) -> BTreeSet<String> {
    payload
        .get("required")
        .and_then(Value::as_array)
        .map(|names| {
            names
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// The kind an outcome names itself, and the fields its `data` actually holds.
fn written(outcome: Outcome) -> (String, BTreeSet<String>) {
    let envelope = outcome.envelope();
    let named = envelope.kind.as_str().to_owned();
    let document = serde_json::to_value(envelope).unwrap_or_default();
    let fields = document
        .pointer("/data")
        .and_then(Value::as_object)
        .map(|fields| fields.keys().cloned().collect())
        .unwrap_or_default();
    (named, fields)
}

/// The committed directory and the types must agree, file for file.
///
/// A change to a serialised shape that forgets to regenerate fails here
/// rather than reaching an SDK.
#[test]
fn the_committed_contract_still_matches_the_types() {
    let apart = layout::differing(&committed(), &fresh());

    assert!(
        apart.is_empty(),
        "the contract is out of date — regenerate it with `just contract`:\n{}",
        apart.join("\n")
    );
}

/// Every reference in the directory, with the file it appears in and the file it
/// names, resolved against the one it appears in.
fn references(files: &Files) -> Vec<(String, String)> {
    fn walk(node: &Value, base: &str, found: &mut Vec<(String, String)>, at: &str) {
        match node {
            Value::Object(fields) => {
                if let Some(Value::String(reference)) = fields.get("$ref") {
                    let named = std::path::Path::new(base).join(reference);
                    found.push((at.to_owned(), normalised(&named)));
                }
                fields
                    .values()
                    .for_each(|value| walk(value, base, found, at));
            }
            Value::Array(items) => items.iter().for_each(|item| walk(item, base, found, at)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    for (path, text) in files {
        let base = path.rsplit_once('/').map_or("", |(dir, _)| dir);
        let document: Value = serde_json::from_str(text).unwrap_or_default();
        walk(&document, base, &mut found, path);
    }
    found
}

/// A path with each `..` taken back out against the part before it.
fn normalised(path: &std::path::Path) -> String {
    let mut parts: Vec<String> = Vec::new();
    for part in path.components() {
        match part {
            std::path::Component::ParentDir => {
                parts.pop();
            }
            std::path::Component::Normal(name) => parts.push(name.to_string_lossy().into_owned()),
            _ => {}
        }
    }
    parts.join("/")
}

/// Every reference names a file the directory holds.
///
/// A reader resolves a `$ref` against the file it appears in, and one naming nothing
/// leaves that reader a field it cannot describe.
#[test]
fn every_reference_names_a_file_the_directory_holds() {
    let files = fresh();
    let dangling: Vec<String> = references(&files)
        .into_iter()
        .filter(|(_, named)| !files.contains_key(named))
        .map(|(at, named)| format!("{at} refers to {named}"))
        .collect();

    assert!(
        !references(&files).is_empty(),
        "the sweep found no reference to check"
    );
    assert!(dangling.is_empty(), "{}", dangling.join("\n"));
}

/// Every file in the directory is something the index names or a definition a named
/// file reaches, so nothing is written that no reader would ever open.
#[test]
fn every_file_is_reached_from_the_index() {
    let files = fresh();
    let index: Value = files
        .get(lemonfiber_api::contract::INDEX)
        .and_then(|text| serde_json::from_str(text).ok())
        .unwrap_or_default();
    let mut reached: BTreeSet<String> =
        BTreeSet::from([lemonfiber_api::contract::INDEX.to_owned()]);
    let mut named = |value: &Value| {
        if let Some(path) = value.as_str() {
            reached.insert(path.to_owned());
        }
    };
    ["key_callable", "reads", "refusals"]
        .iter()
        .filter_map(|key| index.get(key))
        .for_each(&mut named);
    ["kinds", "actions", "bodies"]
        .iter()
        .filter_map(|key| index.get(key).and_then(Value::as_object))
        .flat_map(|listed| listed.values())
        .for_each(&mut named);
    reached.extend(references(&files).into_iter().map(|(_, named)| named));

    let unreached: Vec<&String> = files
        .keys()
        .filter(|path| !reached.contains(*path))
        .collect();
    assert!(unreached.is_empty(), "nothing reaches these: {unreached:?}");
}

/// The contract and the emitters must name the same set of kinds.
///
/// Describing a kind nobody emits, or emitting one the contract omits, are
/// both silent: each half is self-consistent, so only comparing them shows it.
#[test]
fn it_describes_every_kind_that_is_emitted_and_no_others() {
    let contract = Contract::describe();
    let described: Vec<&str> = contract.kinds.keys().map(String::as_str).collect();
    let mut emitted: Vec<&str> = lemonfiber_core::model::kind::ALL
        .iter()
        .map(|kind| kind.as_str())
        .collect();
    emitted.sort_unstable();

    assert_eq!(described, emitted);
}

/// A kind's schema must describe the document that kind writes.
///
/// The two halves are generated from different things — the schema from the
/// report type, the document from `Outcome`'s hand-written `Serialize` — so a
/// variant that starts wrapping its report, or a report whose schema stops
/// tracking it, shows up here rather than in a client that cannot parse the reply.
#[test]
fn each_outcome_is_described_as_the_document_it_writes() {
    let mut seen: HashSet<String> = HashSet::new();
    for outcome in samples() {
        let (kind, fields) = written(outcome);
        let payload = payload(&kind);
        let described = described_fields(&payload);
        let required = required_fields(&payload);

        assert!(
            fields.is_subset(&described),
            "{kind} writes fields the contract does not describe: {fields:?} against {described:?}"
        );
        assert!(
            required.is_subset(&fields),
            "{kind} omits fields the contract requires: {required:?} against {fields:?}"
        );
        seen.insert(kind);
    }

    // Against the set rather than against a number. A count could only ever be
    // compared with itself: a variant added with its sample moved both sides and
    // tripped, and a variant added without one moved neither and passed — which is
    // the case this exists to catch. The set names the kind instead of printing two
    // integers that agree.
    let mut answers: HashSet<String> = HashSet::new();
    Outcome::schemas(|kind, _| {
        answers.insert(kind.to_string());
    });

    let unsampled: BTreeSet<&String> = answers.difference(&seen).collect();
    let unanswered: BTreeSet<&String> = seen.difference(&answers).collect();

    assert!(
        unsampled.is_empty(),
        "these kinds are answers and nothing samples them, so what each writes is \
         compared to nothing: {unsampled:?}"
    );
    assert!(
        unanswered.is_empty(),
        "these kinds are sampled and no answer carries them, so the sample is \
         describing something this never writes: {unanswered:?}"
    );
}

/// Keywords that say something about a schema without constraining what it
/// matches, so they are safe company for a reference.
const ANNOTATIONS: [&str; 4] = ["description", "title", "default", "examples"];

#[test]
fn it_describes_the_wire_version_it_belongs_to() {
    assert_eq!(
        Contract::describe().api_version,
        lemonfiber_core::model::API_VERSION
    );
}

#[test]
fn every_kind_carries_the_whole_envelope_not_just_its_payload() {
    let files = fresh();
    let word: Value = files
        .get("kinds/word.json")
        .and_then(|text| serde_json::from_str(text).ok())
        .unwrap_or_default();

    assert!(word.pointer("/properties/api_version").is_some(), "{word}");
    assert!(word.pointer("/properties/kind").is_some(), "{word}");
    assert!(word.pointer("/properties/data").is_some(), "{word}");
}

#[test]
fn it_is_written_the_same_way_twice() {
    let once = fresh();

    assert_eq!(once, fresh());
    for (path, text) in &once {
        assert!(
            text.ends_with("}\n") || text.ends_with("]\n"),
            "{path} ends {text:?}"
        );
    }
}
