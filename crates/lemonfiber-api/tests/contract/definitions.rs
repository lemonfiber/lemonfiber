//! How the artefact names and nests the shapes it describes.

use serde_json::Value;

use lemonfiber_api::contract::Contract;

/// Every reference in a schema that has a constraint sitting beside it.
///
/// Draft-07 readers discard whatever accompanies a `$ref`; 2020-12 readers
/// apply both. A schema that puts a constraint there therefore means two
/// different things to two readers, and the generators that read this artefact
/// are split across that line — so the artefact must never contain the shape.
fn references_beside_constraints(node: &Value, path: &str, found: &mut Vec<String>) {
    match node {
        Value::Object(fields) => {
            let beside: Vec<&str> = fields
                .keys()
                .map(String::as_str)
                .filter(|key| *key != "$ref" && !ANNOTATIONS.contains(key))
                .collect();
            if fields.contains_key("$ref") && !beside.is_empty() {
                found.push(format!("{path} has {beside:?} beside its $ref"));
            }
            for (key, value) in fields {
                references_beside_constraints(value, &format!("{path}/{key}"), found);
            }
        }
        Value::Array(items) => {
            for (at, item) in items.iter().enumerate() {
                references_beside_constraints(item, &format!("{path}/{at}"), found);
            }
        }
        _ => {}
    }
}

/// The sweep reports the shape it exists to find.
///
/// Without this the sweep below could pass by looking at nothing, which is how
/// the shape it looks for reached two SDKs in the first place.
#[test]
fn a_reference_beside_a_constraint_is_reported() {
    let node = serde_json::json!({
        "oneOf": [{
            "type": "object",
            "$ref": "#/$defs/Problem",
            "properties": { "outcome": { "const": "warn" } }
        }]
    });
    let mut found = Vec::new();
    references_beside_constraints(&node, "", &mut found);

    assert_eq!(found.len(), 1, "{found:?}");
}

/// An annotation is not a constraint, so a described reference is not the shape.
#[test]
fn a_reference_with_only_a_description_is_not_reported() {
    let node = serde_json::json!({
        "description": "The stable identifier for this kind of problem.",
        "$ref": "#/$defs/Code"
    });
    let mut found = Vec::new();
    references_beside_constraints(&node, "", &mut found);

    assert!(found.is_empty(), "{found:?}");
}

/// No kind may describe anything as a reference with a constraint beside it.
///
/// The two readings of that shape cost the same field twice over: one generator
/// keeps the constraint and drops the reference, the other keeps the reference
/// and drops the constraint, and each loses what the other kept.
#[test]
fn no_kind_puts_a_constraint_beside_a_reference() {
    let contract = serde_json::to_value(Contract::describe()).unwrap_or_default();
    let mut found = Vec::new();
    references_beside_constraints(&contract, "", &mut found);

    assert!(found.is_empty(), "{}", found.join(", "));
}

/// Every definition the artefact carries: the kind holding it, its name, and the
/// shape that kind gives it.
///
/// The shape is rendered to text so two copies compare as one value. `serde_json`
/// orders a map's keys, so a definition renders the same way wherever it was found.
fn definitions(contract: &Value) -> Vec<(String, String, String)> {
    contract
        .get("kinds")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(kind, schema)| {
            schema
                .get("$defs")
                .and_then(Value::as_object)
                .map(|named| (kind, named))
        })
        .flat_map(|(kind, named)| {
            named
                .iter()
                .map(move |(name, shape)| (kind.clone(), name.clone(), shape.to_string()))
        })
        .collect()
}

/// A definition name describes one shape, wherever it is carried.
///
/// Twenty-two names did not, because `schemars` names a definition after the bare
/// Rust type and two unrelated types called `Left` are two types with one name. A
/// generator keys a type by that name, so it had to tell them apart itself. Each
/// type carries a name of its own now, and this is what keeps it so.
#[test]
fn a_definition_name_describes_one_shape() {
    let contract = serde_json::to_value(Contract::describe()).unwrap_or_default();
    let mut shapes: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut holding: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (kind, name, shape) in definitions(&contract) {
        shapes.entry(name.clone()).or_default().insert(shape);
        holding.entry(name).or_default().insert(kind);
    }

    // Narrowed by retaining rather than gathered through a closure only a failure
    // would enter. A reporting path nothing walks while the sweep passes is a path
    // nobody has watched work, and it is the half that has to be right on the one
    // day it is read.
    let mut clashing = holding;
    clashing.retain(|name, _| shapes.get(name).is_some_and(|given| given.len() > 1));

    assert!(
        clashing.is_empty(),
        "these definition names describe two different shapes, so anything keying types \
         by name across kinds has to pick one of them: {clashing:?} — give each type a \
         name of its own with `#[schemars(rename = \"...\")]`"
    );
}

/// No definition is numbered to keep it apart from another.
///
/// `schemars` numbers a name it must use twice inside one kind, and the number says
/// where the type was reached rather than anything about it: `Panel4` was the fourth
/// `Panel<T>` the dashboard's fields happened to mention, and reordering those fields
/// renumbered every one of them. A number here is a published type name waiting to
/// move on its own, which is why none may appear rather than none may be added.
///
/// What this cannot reach: `json-schema-to-typescript` adds positional suffixes of
/// its own further downstream — `Remedy1`, `Counted3` — and no name chosen here
/// removes them. This holds the artefact's own numbering, which is the only
/// numbering anything in this repository decides.
#[test]
fn no_definition_is_numbered_apart() {
    let contract = serde_json::to_value(Contract::describe()).unwrap_or_default();
    let numbered: BTreeSet<String> = definitions(&contract)
        .into_iter()
        .map(|(_, name, _)| name)
        .filter(|name| {
            name.chars()
                .next_back()
                .is_some_and(|last| last.is_ascii_digit())
        })
        .collect();

    assert!(
        numbered.is_empty(),
        "`schemars` numbered these to keep two types apart inside one kind, and the \
         number is where the type was reached rather than anything about it: \
         {numbered:?} — give the types names of their own"
    );
}
