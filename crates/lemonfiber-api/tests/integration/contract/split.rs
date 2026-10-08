//! The contract written as the directory it is committed as, from contracts built here.

use std::collections::BTreeMap;

use schemars::Schema;
use serde_json::{json, Value};

use lemonfiber_api::contract::{Contract, INDEX};

/// A contract holding only the kinds given, each as the schema given.
fn holding(kinds: &[(&str, Value)]) -> Contract {
    Contract {
        api_version: 7,
        actions: Vec::new(),
        bodies: BTreeMap::new(),
        key_callable: Vec::new(),
        kinds: kinds
            .iter()
            .map(|(kind, schema)| {
                let Ok(schema) = Schema::try_from(schema.clone()) else {
                    unreachable!("every schema here is an object");
                };
                ((*kind).to_owned(), schema)
            })
            .collect(),
        reads: Vec::new(),
        refusals: BTreeMap::new(),
    }
}

/// `value` with whatever sits at `pointer` replaced by `with`.
fn replaced(mut value: Value, pointer: &str, with: Value) -> Value {
    if let Some(slot) = value.pointer_mut(pointer) {
        *slot = with;
    }
    value
}

/// The one fault a refusal names, or nothing where it names none or several.
fn only(faults: &[String]) -> &str {
    match faults {
        [one] => one,
        _ => "",
    }
}

/// One file of the directory, parsed.
fn file(files: &BTreeMap<String, String>, path: &str) -> Value {
    files
        .get(path)
        .and_then(|text| serde_json::from_str(text).ok())
        .unwrap_or_default()
}

const DIALECT: &str = "https://json-schema.org/draft/2020-12/schema";

/// A kind whose payload is `Remedy`, which refers on to `Code`.
fn carrying_remedy() -> Value {
    json!({
        "$schema": DIALECT,
        "properties": { "data": { "$ref": "#/$defs/Remedy" } },
        "$defs": {
            "Remedy": { "properties": { "code": { "$ref": "#/$defs/Code" } } },
            "Code": { "type": "string" }
        }
    })
}

#[test]
fn a_definition_two_kinds_carry_is_written_once_and_reached_from_both() {
    let contract = holding(&[("doctor", carrying_remedy()), ("repair", carrying_remedy())]);
    let Ok(files) = contract.files() else {
        unreachable!("two kinds agreeing on a definition is the ordinary case");
    };

    let defs: Vec<&String> = files
        .keys()
        .filter(|path| path.starts_with("defs/"))
        .collect();
    assert_eq!(defs, ["defs/Code.json", "defs/Remedy.json"]);
    for kind in ["kinds/doctor.json", "kinds/repair.json"] {
        let envelope = file(&files, kind);
        assert_eq!(
            envelope.pointer("/properties/data/$ref"),
            Some(&json!("../defs/Remedy.json"))
        );
        assert_eq!(
            envelope.get("$defs"),
            None,
            "{kind} still carries its definitions"
        );
    }
}

#[test]
fn a_definition_reaches_another_by_its_file_name_and_names_its_dialect() {
    let Ok(files) = holding(&[("doctor", carrying_remedy())]).files() else {
        unreachable!("one kind cannot disagree with itself");
    };
    let remedy = file(&files, "defs/Remedy.json");

    assert_eq!(
        remedy.pointer("/properties/code/$ref"),
        Some(&json!("Code.json"))
    );
    assert_eq!(remedy.get("$schema"), Some(&json!(DIALECT)));
}

#[test]
fn the_index_carries_the_version_and_names_every_other_file() {
    let Ok(files) = holding(&[("doctor", carrying_remedy())]).files() else {
        unreachable!("one kind cannot disagree with itself");
    };
    let index = file(&files, INDEX);

    assert_eq!(
        index,
        json!({
            "api_version": 7,
            "actions": {},
            "bodies": {},
            "key_callable": "key-callable.json",
            "kinds": { "doctor": "kinds/doctor.json" },
            "reads": "reads.json",
            "refusals": "refusals.json"
        })
    );
    for named in [
        "key-callable.json",
        "reads.json",
        "refusals.json",
        "kinds/doctor.json",
    ] {
        assert!(
            files.contains_key(named),
            "the index names {named} and nothing wrote it"
        );
    }
}

#[test]
fn a_definition_two_kinds_describe_differently_is_refused_by_name() {
    let other = replaced(
        carrying_remedy(),
        "/$defs/Code",
        json!({ "type": "integer" }),
    );
    let refused = holding(&[("doctor", carrying_remedy()), ("repair", other)]).files();

    let Err(faults) = refused else {
        unreachable!("one file cannot hold two shapes");
    };
    assert!(only(&faults).starts_with("Code is described"), "{faults:?}");
}

#[test]
fn a_reference_to_anything_but_a_definition_beside_it_is_refused() {
    let reaching = replaced(
        carrying_remedy(),
        "/properties/data",
        json!({ "$ref": "#/$defs/Remedy/properties/code" }),
    );
    let refused = holding(&[("doctor", reaching)]).files();

    let Err(faults) = refused else {
        unreachable!("a path into a definition has no file to point at");
    };
    assert!(
        only(&faults).contains("#/$defs/Remedy/properties/code"),
        "{faults:?}"
    );
}

#[test]
fn kinds_written_in_two_dialects_are_refused() {
    let older = replaced(
        carrying_remedy(),
        "/$schema",
        json!("http://json-schema.org/draft-07/schema#"),
    );
    let refused = holding(&[("doctor", carrying_remedy()), ("repair", older)]).files();

    let Err(faults) = refused else {
        unreachable!("a shared definition cannot name two dialects");
    };
    assert!(
        only(&faults).starts_with("kinds/repair.json is written in"),
        "{faults:?}"
    );
}

#[test]
fn a_definition_names_no_dialect_where_no_kind_does() {
    let mut bare = carrying_remedy();
    if let Some(object) = bare.as_object_mut() {
        object.remove("$schema");
    }
    let Ok(files) = holding(&[("doctor", bare)]).files() else {
        unreachable!("one kind cannot disagree with itself");
    };

    assert_eq!(file(&files, "defs/Code.json"), json!({ "type": "string" }));
}

/// A body is written under its route, its definitions moved beside the kinds' and
/// pointed at there, and the index names the file by the route.
#[test]
fn a_body_is_filed_under_its_route_with_its_definitions_shared() {
    let mut contract = holding(&[("doctor", carrying_remedy())]);
    let Ok(body) = Schema::try_from(json!({
        "type": "object",
        "properties": { "choice": { "$ref": "#/$defs/Choice" } },
        "$defs": { "Choice": { "type": "string", "enum": ["resume"] } }
    })) else {
        unreachable!("the body is an object");
    };
    contract
        .bodies
        .insert("/api/setup/recover".to_owned(), body);
    let Ok(files) = contract.files() else {
        unreachable!("one body cannot disagree with itself");
    };
    let index = file(&files, INDEX);
    assert_eq!(
        index.pointer("/bodies/~1api~1setup~1recover"),
        Some(&json!("bodies/setup-recover.json"))
    );
    let written = file(&files, "bodies/setup-recover.json");
    assert_eq!(
        written.pointer("/properties/choice/$ref"),
        Some(&json!("../defs/Choice.json"))
    );
    assert!(files.contains_key("defs/Choice.json"));
}
