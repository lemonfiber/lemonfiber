//! The contract as the directory it is committed as.
//!
//! One file per kind, one file per definition, and the lists that are not kinds in
//! files of their own, all named by an index. Every definition sits in `defs/`
//! whether one kind carries it or nine: a definition shared by several kinds is then
//! written once, a kind's file is its envelope and nothing else, and every reference
//! in the directory has one form, a path to a file.

use std::collections::BTreeMap;

use schemars::Schema;
use serde::Serialize;
use serde_json::Value;

use super::layout::{rendered, Files};
use super::path::INDEX;
use super::Contract;

/// Where the kinds are, inside the contract's directory.
const KINDS: &str = "kinds";

/// Where the definitions are, inside the contract's directory.
const DEFS: &str = "defs";

/// The actions a key may call, inside the contract's directory.
const KEY_CALLABLE: &str = "key-callable.json";

/// Where each action the surface takes is listed, a file per action, inside the
/// contract's directory.
const ACTIONS: &str = "actions";

/// The reads the surface serves, inside the contract's directory.
const READS: &str = "reads.json";

/// The refusals a request may meet, inside the contract's directory.
const REFUSALS: &str = "refusals.json";

/// The one reference form `schemars` writes: a definition carried beside the schema.
const LOCAL: &str = "#/$defs/";

/// What the index says: the wire version, and where everything else is.
#[derive(Debug, Serialize)]
struct Index {
    /// The wire version every file in the directory belongs to.
    api_version: u32,
    /// Each action the surface takes, to the file listing it.
    actions: BTreeMap<String, String>,
    /// The file listing every action a key may call.
    key_callable: &'static str,
    /// `kind` to the file holding the schema of the envelope carrying it.
    kinds: BTreeMap<String, String>,
    /// The file listing every read the surface serves.
    reads: &'static str,
    /// The file listing every code a refusal may carry.
    refusals: &'static str,
}

impl Contract {
    /// Every file of the contract's directory, keyed by its path inside it.
    ///
    /// # Errors
    ///
    /// Every fault that would make the directory describe something other than the
    /// types: a definition two kinds describe differently, which one file cannot hold,
    /// a reference in a form this does not rewrite, which would point at nothing once
    /// the definitions are no longer beside it, and kinds written in two dialects.
    pub fn files(&self) -> Result<Files, Vec<String>> {
        let mut files = Files::new();
        let mut faults = Vec::new();
        let mut defs: BTreeMap<String, Value> = BTreeMap::new();
        let mut kinds = BTreeMap::new();
        let mut written_in: Option<Value> = None;

        for (kind, schema) in &self.kinds {
            let path = format!("{KINDS}/{kind}.json");
            let mut envelope = schema.clone();
            match (&written_in, envelope.get("$schema")) {
                (None, Some(dialect)) => written_in = Some(dialect.clone()),
                (Some(held), Some(dialect)) if held != dialect => faults.push(format!(
                    "{path} is written in {dialect} and a kind before it in {held}, so the \
                     definitions they share have no one dialect to name"
                )),
                _ => {}
            }
            if let Some(Value::Object(carried)) = envelope.remove("$defs") {
                for (name, shape) in carried {
                    match defs.get(&name) {
                        Some(held) if *held != shape => faults.push(format!(
                            "{name} is described one way by {kind} and another way by a kind \
                             before it, and one file can hold only one of them"
                        )),
                        Some(_) => {}
                        None => {
                            defs.insert(name, shape);
                        }
                    }
                }
            }
            for value in envelope
                .as_object_mut()
                .into_iter()
                .flat_map(|object| object.values_mut())
            {
                pointed(value, "../defs/", &path, &mut faults);
            }
            put(&mut files, &path, &envelope);
            kinds.insert(kind.clone(), path);
        }

        for (name, mut shape) in defs {
            let path = format!("{DEFS}/{name}.json");
            pointed(&mut shape, "", &path, &mut faults);
            // A definition is an object or a boolean, as `schemars` writes every one, so
            // it is always a schema; the empty one on the impossible branch keeps this
            // free of a line no test can reach.
            let schema = Schema::try_from(shape).unwrap_or_default();
            put(&mut files, &path, &dialect(schema, written_in.as_ref()));
        }

        let mut actions = BTreeMap::new();
        for action in &self.actions {
            let path = format!("{ACTIONS}/{}.json", action.action);
            put(&mut files, &path, action);
            actions.insert(action.action.to_owned(), path);
        }
        put(&mut files, KEY_CALLABLE, &self.key_callable);
        put(&mut files, READS, &self.reads);
        put(&mut files, REFUSALS, &self.refusals);
        let index = Index {
            api_version: self.api_version,
            actions,
            key_callable: KEY_CALLABLE,
            kinds,
            reads: READS,
            refusals: REFUSALS,
        };
        put(&mut files, INDEX, &index);

        if faults.is_empty() {
            Ok(files)
        } else {
            Err(faults)
        }
    }
}

/// One file into the set.
fn put<T: Serialize + ?Sized>(files: &mut Files, path: &str, value: &T) {
    files.insert(path.to_owned(), rendered(value));
}

/// A definition as a document of its own, naming the dialect the kinds are written in.
///
/// A reader handed one file has nothing else to learn the dialect from, and the two
/// dialects the generators reading this are split across read a `$ref` differently.
fn dialect(mut schema: Schema, written_in: Option<&Value>) -> Schema {
    if let Some(dialect) = written_in {
        schema.insert("$schema".to_owned(), dialect.clone());
    }
    schema
}

/// Every reference under `node` rewritten from the definition beside it to the
/// definition's own file, reached from `to_defs`.
fn pointed(node: &mut Value, to_defs: &str, path: &str, faults: &mut Vec<String>) {
    match node {
        Value::Object(fields) => {
            if let Some(Value::String(reference)) = fields.get_mut("$ref") {
                match reference.strip_prefix(LOCAL) {
                    Some(name) if !name.is_empty() && !name.contains('/') => {
                        *reference = format!("{to_defs}{name}.json");
                    }
                    _ => faults.push(format!(
                        "{path} refers to {reference}, which is not a definition this \
                         directory holds a file for"
                    )),
                }
            }
            for value in fields.values_mut() {
                if !value.is_string() {
                    pointed(value, to_defs, path, faults);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                pointed(item, to_defs, path, faults);
            }
        }
        _ => {}
    }
}
