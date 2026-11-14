//! The previous release's contract artefact, committed by the version bump, and what this
//! build's artefact added, removed or changed against it.

use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use super::layout::{self, Files};
use super::path::INDEX;

/// Where the previous release's artefact is committed, relative to the workspace root.
pub const RELEASED_DIR: &str = "contract/released";

/// The file in that directory naming the release it was copied from.
pub const VERSION: &str = "VERSION";

/// Where the difference is written, relative to the workspace root.
pub const DIFF: &str = "contract/contract-diff.json";

/// The directory an artefact is a copy of, inside [`RELEASED_DIR`].
const DIRECTORY: &str = "web-api";

/// The one file an artefact was, before it was a directory, inside [`RELEASED_DIR`].
const SINGLE: &str = "web-api.contract.json";

/// Every part of an artefact a release can add, remove or change, each by its name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Snapshot {
    /// Each kind, to the schema of the envelope carrying it.
    pub kinds: BTreeMap<String, Value>,
    /// Each definition, to its schema.
    pub definitions: BTreeMap<String, Value>,
    /// Each read, by its path.
    pub reads: BTreeMap<String, Value>,
    /// Each action, by its name.
    pub actions: BTreeMap<String, Value>,
    /// Each refusal, by its code.
    pub refusals: BTreeMap<String, Value>,
    /// Each kind of event the stream says.
    pub events: BTreeMap<String, Value>,
}

impl Snapshot {
    /// The artefact as the directory `files` holds.
    #[must_use]
    pub fn of(files: &Files) -> Self {
        let parsed = |path: &str| -> Value {
            files
                .get(path)
                .and_then(|text| serde_json::from_str(text).ok())
                .unwrap_or(Value::Null)
        };
        let index = parsed(INDEX);
        let named = |key: &str| -> BTreeMap<String, Value> {
            listed(index.get(key))
                .into_iter()
                .filter_map(|(name, path)| Some((name, parsed(path.as_str()?))))
                .collect()
        };
        let mut refusals = BTreeMap::new();
        for family in named("refusals").into_values() {
            refusals.extend(listed(Some(&family)));
        }
        Self {
            kinds: named("kinds")
                .into_iter()
                .map(|(name, schema)| (name, plain(schema)))
                .collect(),
            definitions: files
                .keys()
                .filter_map(|path| {
                    Some((
                        path.strip_prefix("defs/")?
                            .strip_suffix(".json")?
                            .to_owned(),
                        plain(parsed(path)),
                    ))
                })
                .collect(),
            reads: by_path(
                index
                    .get("reads")
                    .and_then(Value::as_str)
                    .map(parsed)
                    .as_ref(),
            ),
            actions: named("actions"),
            refusals,
            events: listed(index.pointer("/events/kinds")),
        }
    }

    /// The artefact as the one file it was before it was a directory: each kind carrying
    /// its own definitions, and no actions or events.
    #[must_use]
    pub fn of_single(document: &Value) -> Self {
        let mut definitions = BTreeMap::new();
        let mut kinds = BTreeMap::new();
        for (name, mut schema) in listed(document.get("kinds")) {
            let carried = schema
                .as_object_mut()
                .and_then(|schema| schema.remove("$defs"));
            for (definition, shape) in listed(carried.as_ref()) {
                definitions.insert(definition, plain(shape));
            }
            kinds.insert(name, plain(schema));
        }
        Self {
            kinds,
            definitions,
            reads: by_path(document.get("reads")),
            actions: BTreeMap::new(),
            refusals: listed(document.get("refusals")),
            events: BTreeMap::new(),
        }
    }

    /// The artefact committed under `dir`, as a directory or as the one file it once was.
    ///
    /// # Errors
    ///
    /// Where `dir` holds neither, or what it holds cannot be read.
    pub fn read(dir: &Path) -> io::Result<Self> {
        let directory = dir.join(DIRECTORY);
        if directory.is_dir() {
            return layout::read(&directory).map(|files| Self::of(&files));
        }
        let text = std::fs::read_to_string(dir.join(SINGLE))?;
        serde_json::from_str(&text)
            .map(|document| Self::of_single(&document))
            .map_err(io::Error::other)
    }
}

/// What one part of the artefact gained, lost and changed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Moved {
    /// Names this release has that the previous one did not.
    pub added: Vec<String>,
    /// Names the previous release had that this one does not.
    pub removed: Vec<String>,
    /// Names both have, described differently.
    pub changed: Vec<String>,
}

/// What this release's artefact added, removed or changed against the previous release's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diff {
    /// The release compared against.
    pub from: String,
    /// The kinds.
    pub kinds: Moved,
    /// The definitions.
    pub definitions: Moved,
    /// The reads, by path.
    pub reads: Moved,
    /// The actions.
    pub actions: Moved,
    /// The refusals, by code.
    pub refusals: Moved,
    /// The kinds of event the stream says.
    pub events: Moved,
}

impl Diff {
    /// What `now` added, removed or changed against `before`, the artefact `from` released.
    #[must_use]
    pub fn between(from: &str, before: &Snapshot, now: &Snapshot) -> Self {
        Self {
            from: from.to_owned(),
            kinds: moved(&before.kinds, &now.kinds),
            definitions: moved(&before.definitions, &now.definitions),
            reads: moved(&before.reads, &now.reads),
            actions: moved(&before.actions, &now.actions),
            refusals: moved(&before.refusals, &now.refusals),
            events: moved(&before.events, &now.events),
        }
    }
}

/// What `now` gained, lost and changed against `before`.
fn moved(before: &BTreeMap<String, Value>, now: &BTreeMap<String, Value>) -> Moved {
    Moved {
        added: now
            .keys()
            .filter(|name| !before.contains_key(*name))
            .cloned()
            .collect(),
        removed: before
            .keys()
            .filter(|name| !now.contains_key(*name))
            .cloned()
            .collect(),
        changed: now
            .iter()
            .filter(|(name, value)| before.get(*name).is_some_and(|held| held != *value))
            .map(|(name, _)| name.clone())
            .collect(),
    }
}

/// An object's members, or nothing where it is not one.
fn listed(value: Option<&Value>) -> BTreeMap<String, Value> {
    value
        .and_then(Value::as_object)
        .map(|members| members.clone().into_iter().collect())
        .unwrap_or_default()
}

/// A list of reads, keyed by each one's path.
fn by_path(reads: Option<&Value>) -> BTreeMap<String, Value> {
    reads
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|read| Some((read.get("path")?.as_str()?.to_owned(), read.clone())))
        .collect()
}

/// A schema with the dialect it names left out and every reference written as the bare
/// name of the definition it reaches, so where a definition is kept is not a change.
fn plain(mut schema: Value) -> Value {
    if let Some(fields) = schema.as_object_mut() {
        fields.remove("$schema");
    }
    named(&mut schema);
    schema
}

/// Every reference under `node` written as the bare name of the definition it reaches.
fn named(node: &mut Value) {
    match node {
        Value::Object(fields) => {
            if let Some(Value::String(target)) = fields.get_mut("$ref") {
                let name = target.rsplit('/').next().unwrap_or(target);
                *target = name.strip_suffix(".json").unwrap_or(name).to_owned();
            }
            fields.values_mut().for_each(named);
        }
        Value::Array(items) => items.iter_mut().for_each(named),
        _ => {}
    }
}
