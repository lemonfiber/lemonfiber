//! Every secret an answer the arms read holds, withheld wherever the answer is printed.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::live::core::{secrets_in, shown, SECRETS, WITHHELD};

/// Where the contract keeps each kind's schema and example.
const CONTRACT: &str = "contract/web-api";

/// What each secret is written as in the example it is placed in, with what JSON escapes.
const PLACED: &str = r#"a "placed" value\"#;

/// The word of [`PLACED`] that is printed wherever any of it is.
const PLACED_WORD: &str = "placed";

/// What a container's log is printed through, as the workflow writes it.
const FILTERED: &str = "| perl -pe \"${WITHHELD}\"";

/// How many references a field is followed through before it is called undeclared.
const REFERENCES: usize = 8;

/// The JSON document at `path`, or why it does not read.
fn json(path: &Path) -> Result<Value, String> {
    let written =
        std::fs::read_to_string(path).map_err(|why| format!("{}: {why}", path.display()))?;
    serde_json::from_str(&written).map_err(|why| format!("{}: {why}", path.display()))
}

/// `document`, with `secret` written at each of `path` in turn, each field on the way
/// made where it is not.
fn placed(document: Value, path: &[&str], secret: &str) -> Value {
    let Some((first, rest)) = path.split_first() else {
        return Value::String(secret.to_owned());
    };
    let mut fields = match document {
        Value::Object(fields) => fields,
        _ => serde_json::Map::new(),
    };
    let within = fields.remove(*first).unwrap_or(Value::Null);
    fields.insert((*first).to_owned(), placed(within, rest, secret));
    Value::Object(fields)
}

/// Each schema `schema` stands for: the one it refers to, and each it may be.
fn alternatives(schema: &Value, within: &Path) -> Vec<(Value, PathBuf)> {
    let referred = schema
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|reference| reference.split('#').next())
        .filter(|file| !file.is_empty())
        .map(|file| within.join(file))
        .and_then(|file| {
            let parent = file.parent()?.to_path_buf();
            Some((json(&file).ok()?, parent))
        });
    let either = ["anyOf", "oneOf", "allOf"]
        .iter()
        .filter_map(|key| schema.get(*key).and_then(Value::as_array))
        .flatten()
        .map(|one| (one.clone(), within.to_path_buf()));
    referred.into_iter().chain(either).collect()
}

/// Whether `schema`, read beside `within`, declares a field at each of `path` in turn.
fn declares(schema: &Value, within: &Path, path: &[&str], left: usize) -> bool {
    let Some((first, rest)) = path.split_first() else {
        return true;
    };
    left > 0
        && (schema
            .get("properties")
            .and_then(|fields| fields.get(*first))
            .is_some_and(|field| declares(field, within, rest, left))
            || alternatives(schema, within)
                .iter()
                .any(|(one, beside)| declares(one, beside, path, left - 1)))
}

#[test]
fn every_secret_withheld_is_where_its_kind_declares_it() {
    let contract = crate::root().join(CONTRACT);
    for (kind, at) in SECRETS {
        let schema = json(&contract.join("kinds").join(format!("{kind}.json")));
        assert!(schema.is_ok(), "{schema:?}");
        let path: Vec<&str> = at.split('/').skip(1).collect();
        assert!(
            schema.is_ok_and(|schema| declares(
                &schema,
                &contract.join("kinds"),
                &path,
                REFERENCES
            )),
            "{kind} declares nothing at {at}"
        );
    }
}

#[test]
fn an_answer_holding_a_secret_is_printed_with_it_withheld() {
    let examples = crate::root().join(CONTRACT).join("examples/kinds");
    for (kind, at) in SECRETS {
        let example = json(&examples.join(format!("{kind}.json")));
        assert!(example.is_ok(), "{example:?}");
        let path: Vec<&str> = at.split('/').skip(1).collect();
        let said = placed(example.unwrap_or_default(), &path, PLACED);
        let held = secrets_in(&said);
        assert_eq!(held, secrets_in(&Value::Array(vec![said.clone()])));
        let printed = shown(
            said.to_string(),
            &held.iter().map(String::as_str).collect::<Vec<_>>(),
        );
        assert!(
            !printed.contains(PLACED_WORD) && printed.contains(WITHHELD),
            "{kind} at {at} printed whole"
        );
    }
}

#[test]
fn every_container_log_the_workflow_prints_is_printed_through_the_filter() {
    let written = std::fs::read_to_string(crate::root().join(crate::WORKFLOW));
    assert!(written.is_ok(), "{written:?}");
    let logs: Vec<String> = written
        .unwrap_or_default()
        .lines()
        .filter(|line| line.contains("docker logs"))
        .map(str::to_owned)
        .collect();
    assert!(!logs.is_empty(), "the workflow prints no container's log");
    for line in logs {
        assert!(line.contains(FILTERED), "printed unfiltered: {line}");
    }
}
