//! The contract artefact describes exactly what this build writes.
//!
//! Every kind is described, every outcome is described as the document it writes,
//! the committed artefacts match what the types generate, and the stable surface a
//! consumer relies on never loses anything under an unchanged wire version.

mod definitions;
mod samples;
mod surface;

use std::collections::{BTreeSet, HashSet};

use serde_json::Value;

use lemonfiber_api::contract::{Contract, CONTRACT_PATH};
use lemonfiber_core::app::Outcome;

use samples::samples;

/// What is committed, read from the workspace root.
fn committed() -> Option<String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read_to_string(root.join(CONTRACT_PATH)).ok()
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

/// How much of each rendering is shown either side of the first difference.
const AROUND: usize = 140;

/// Where two renderings of the contract first part company, in words.
///
/// Nothing where they agree. A gate saying only that the artefact is stale costs
/// whoever reads it a whole regeneration to find out what moved, and the answer is
/// already in the two strings it is holding.
fn differing(stored: &str, fresh: &str) -> String {
    let alike = stored
        .chars()
        .zip(fresh.chars())
        .take_while(|(held, made)| held == made)
        .count();
    if alike == stored.chars().count() && alike == fresh.chars().count() {
        return String::new();
    }
    let held: String = stored.chars().skip(alike).take(AROUND).collect();
    let made: String = fresh.chars().skip(alike).take(AROUND).collect();
    format!(
        " — they part company {alike} characters in: the file has {held:?} where the \
         types make {made:?}"
    )
}

/// The committed artefact and the types must agree.
///
/// A change to a serialised shape that forgets to regenerate fails here
/// rather than reaching an SDK.
#[test]
fn the_committed_contract_still_matches_the_types() {
    let fresh = Contract::describe().to_json().unwrap_or_default();
    let stored = committed().unwrap_or_default();
    // Bound rather than written into the assertion's own message, which is
    // evaluated only where the assertion fails — and a rendering nothing runs is
    // a rendering nothing holds to being readable.
    let apart = differing(&stored, &fresh);

    assert_eq!(
        stored, fresh,
        "the contract is out of date — regenerate it with `just contract`{apart}"
    );
}

/// Two renderings that agree say nothing, and two that do not say where.
#[test]
fn what_a_stale_artefact_is_told_is_where_it_went_wrong() {
    assert_eq!(differing("the same", "the same"), String::new());

    let apart = differing("the same up to here", "the same up to there");
    assert!(apart.contains("15 characters in"), "{apart}");
    assert!(apart.contains("\"here\""), "{apart}");
    assert!(apart.contains("\"there\""), "{apart}");

    let shorter = differing("the same", "the same and more");
    assert!(shorter.contains("8 characters in"), "{shorter}");
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
    let contract = Contract::describe();
    let text = contract.to_json().unwrap_or_default();

    assert!(contract.kinds.contains_key("word"), "{:?}", contract.kinds);
    assert!(text.contains("api_version"), "{text}");
    assert!(text.contains("kind"), "{text}");
}

#[test]
fn it_is_written_the_same_way_twice() {
    let once = Contract::describe().to_json();
    let twice = Contract::describe().to_json();

    assert_eq!(once, twice);
    assert!(once.unwrap_or_default().ends_with("}\n"));
}
