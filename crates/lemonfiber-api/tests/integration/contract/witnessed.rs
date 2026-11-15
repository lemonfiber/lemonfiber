//! The examples the contract carries: one of every kind, every action and every read,
//! each holding to the schema it is an example of.

use std::collections::BTreeSet;

use boon::{Compiler, Schemas};
use serde_json::{json, Map, Value};

use lemonfiber_api::contract::example::{Acted, Examples};
use lemonfiber_api::contract::layout::Files;
use lemonfiber_api::contract::{Contract, INDEX};

use super::{directory, fresh};

/// Where the directory's files are placed for the validator, which resolves a `$ref`
/// against the file it appears in.
const BASE: &str = "file:///web-api/";

/// One file of the directory, parsed.
fn parsed(files: &Files, path: &str) -> Value {
    files
        .get(path)
        .and_then(|text| serde_json::from_str(text).ok())
        .unwrap_or_default()
}

/// What `instance` breaks of the schema at `at`, compiled from what `compiler` holds.
fn broken(
    compiler: &mut Compiler,
    schemas: &mut Schemas,
    at: &str,
    instance: &Value,
) -> Option<String> {
    match compiler.compile(&format!("{BASE}{at}"), schemas) {
        Ok(index) => schemas
            .validate(instance, index)
            .err()
            .map(|error| format!("{at}: {error}")),
        Err(error) => Some(format!("{at}: {error}")),
    }
}

/// The schema an action's request body is held to: each argument it lists, of the type
/// it lists, and nothing else.
fn arguments_of(listed: &Value) -> Value {
    let properties: Map<String, Value> = listed
        .get("arguments")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|argument| {
            Some((
                argument.get("name")?.as_str()?.to_owned(),
                argument.get("type")?.clone(),
            ))
        })
        .collect();
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "properties": properties,
        "additionalProperties": false,
    })
}

/// Every way an example in `files` fails the schema it is an example of: a kind's
/// against that kind's envelope, an action's arguments against the arguments it lists,
/// and what it answered against the kind it answered with.
fn faults(files: &Files) -> Vec<String> {
    let mut compiler = Compiler::new();
    let mut found = Vec::new();
    for path in files
        .keys()
        .filter(|path| path.starts_with("kinds/") || path.starts_with("defs/"))
    {
        if let Err(error) = compiler.add_resource(&format!("{BASE}{path}"), parsed(files, path)) {
            found.push(format!("{path}: {error}"));
        }
    }
    let mut schemas = Schemas::new();
    for (path, _) in files
        .iter()
        .filter(|(path, _)| path.starts_with("examples/"))
    {
        let example = parsed(files, path);
        let name = path
            .rsplit_once('/')
            .map_or("", |(_, file)| file.trim_end_matches(".json"));
        let (instance, at) = if path.starts_with("examples/kinds/") {
            if example.get("kind").and_then(Value::as_str) != Some(name) {
                found.push(format!("{path} is an envelope of another kind"));
            }
            (example, format!("kinds/{name}.json"))
        } else {
            let listed = parsed(files, &format!("actions/{name}.json"));
            let at = format!("arguments/{name}.json");
            if let Err(error) = compiler.add_resource(&format!("{BASE}{at}"), arguments_of(&listed))
            {
                found.push(format!("{at}: {error}"));
            }
            let arguments = example.get("arguments").cloned().unwrap_or_default();
            found.extend(broken(&mut compiler, &mut schemas, &at, &arguments));
            let answered = example.get("answered").cloned().unwrap_or_default();
            let kind = answered
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            (answered, format!("kinds/{kind}.json"))
        };
        found.extend(broken(&mut compiler, &mut schemas, &at, &instance));
    }
    found
}

/// The names an index entry maps to files.
fn named(index: &Value, pointer: &str) -> BTreeSet<String> {
    index
        .pointer(pointer)
        .and_then(Value::as_object)
        .map(|listed| listed.keys().cloned().collect())
        .unwrap_or_default()
}

/// Every example the harness wrote holds to its schema.
///
/// An example that has stopped being true fails here rather than in a reader who
/// copied it.
#[test]
fn every_example_holds_to_its_schema() {
    let files = fresh();
    let found = faults(&files);

    assert!(
        files
            .keys()
            .filter(|path| path.starts_with("examples/"))
            .count()
            > 100,
        "the harness wrote too few examples to be checking anything"
    );
    assert!(found.is_empty(), "{}", found.join("\n"));
}

/// Every kind, every action and every kind a read answers with that is not a file has
/// an example, and the index names each.
#[test]
fn every_kind_action_and_read_has_an_example() {
    let files = fresh();
    let index = parsed(&files, INDEX);
    let contract = Contract::describe();
    let kinds = named(&index, "/examples/kinds");
    let actions = named(&index, "/examples/actions");
    let every_kind: BTreeSet<String> = lemonfiber_core::model::kind::ALL
        .iter()
        .map(|kind| kind.as_str().to_owned())
        .collect();
    let every_action: BTreeSet<String> = contract
        .actions
        .iter()
        .map(|action| action.action.to_owned())
        .collect();
    let read_kinds: BTreeSet<String> = serde_json::to_value(&contract.reads)
        .unwrap_or_default()
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|read| {
            read.get("kinds")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        })
        .filter_map(|kind| kind.as_str().map(str::to_owned))
        .collect();

    let unexampled: Vec<&String> = every_kind.difference(&kinds).collect();
    assert!(
        unexampled.is_empty(),
        "no example of these kinds: {unexampled:?}"
    );
    let untaken: Vec<&String> = every_action.difference(&actions).collect();
    assert!(
        untaken.is_empty(),
        "no example of these actions: {untaken:?}"
    );
    let unread: Vec<&String> = read_kinds.difference(&kinds).collect();
    assert!(
        unread.is_empty(),
        "no example of what these reads answer: {unread:?}"
    );
}

/// An example that does not hold to its schema is refused: a kind whose payload is
/// something else, an argument the action does not list, and an answer missing what
/// its kind requires.
#[test]
fn an_example_that_does_not_hold_is_refused() {
    let examples = Examples {
        kinds: [(
            "version".to_owned(),
            json!({ "api_version": 1, "kind": "version", "data": "a version" }),
        )]
        .into(),
        actions: [(
            "up".to_owned(),
            Acted {
                arguments: [("everything".to_owned(), json!(true))]
                    .into_iter()
                    .collect(),
                answered: json!({ "api_version": 1, "kind": "job", "data": {} }),
            },
        )]
        .into(),
    };
    let found = faults(&directory(&Contract::describe().with_examples(examples)));

    assert_eq!(found.len(), 3, "{}", found.join("\n"));
    assert!(
        found
            .iter()
            .any(|fault| fault.starts_with("kinds/version.json")),
        "{found:?}"
    );
    assert!(
        found
            .iter()
            .any(|fault| fault.starts_with("arguments/up.json")),
        "{found:?}"
    );
    assert!(
        found
            .iter()
            .any(|fault| fault.starts_with("kinds/job.json")),
        "{found:?}"
    );
}
