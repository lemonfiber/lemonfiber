//! Whether a manifest conforms to the schema this build publishes.
//!
//! The schema is generated from the very types a manifest is read into, so this is the
//! reader holding a file to a description of itself. A table of fields written out here
//! would be a second account of what a plugin may say, free to disagree with the parser
//! about what is required — and the disagreement surfaces as a manifest that validates
//! in an author's editor and is refused on an operator's machine.
//!
//! It exists because a strict deserialiser answers one fault at a time, and a
//! third-party manifest is far likelier to carry several than a first-party one. Fixing
//! a file one error per run is a guessing game, so everything a typed read would stop at
//! — a field this build does not declare, a required field that is absent, a value of
//! the wrong kind, a word outside a closed set — is collected here first, each placed
//! where the manifest put it.
//!
//! A fault is placed by the id its entry declares rather than by position, because that
//! is what the author is looking at. An entry with no id yet is still placed, by
//! position: a `[[service]]` can be missing its id and say something else wrong in the
//! same breath, and "the fourth service" beats no location at all.

use std::collections::BTreeSet;

use schemars::{schema_for, Schema};
use serde_json::Value as Json;
use toml::Value as Toml;

use crate::schema::Manifest;

/// One thing a manifest got wrong, and where.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Violation {
    /// Which declaration it is about, in the manifest's own terms.
    pub location: String,
    /// What is wrong with it.
    pub message: String,
}

impl std::fmt::Display for Violation {
    fn fmt(&self, into: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.location.is_empty() {
            return write!(into, "{}", self.message);
        }
        write!(into, "{}: {}", self.location, self.message)
    }
}

/// The schema this build publishes, generated from the types the reader uses.
fn published() -> Schema {
    schema_for!(Manifest)
}

/// Everything the published schema refuses about this text, in one pass.
///
/// Silent on a file that is not TOML at all: there is nothing to walk, and the typed
/// read that follows describes that failure far better than a walk could.
pub(crate) fn nonconforming(text: &str) -> Vec<Violation> {
    let Ok(tree) = toml::from_str::<Toml>(text) else {
        return Vec::new();
    };
    let published = published();
    let schema = published.as_value();
    let mut found = Vec::new();
    against(&tree, schema, schema, "", &mut found);
    found
}

/// One value against one schema, and everything that does not hold.
fn against(value: &Toml, schema: &Json, root: &Json, at: &str, found: &mut Vec<Violation>) {
    let schema = resolved(schema, root);

    if let Some(branches) = branches(schema) {
        alternatives(value, branches, root, at, found);
        return;
    }
    if let Some(only) = schema.get("const") {
        if !is(value, only) {
            found.push(Violation {
                location: at.to_owned(),
                message: format!("{} is not {}", said(value), rendered(only)),
            });
        }
        return;
    }
    if !kinds(schema).is_empty() && !kinds(schema).iter().any(|kind| holds(value, kind)) {
        found.push(Violation {
            location: at.to_owned(),
            message: format!(
                "expected {}, found {}",
                wanted(&kinds(schema)),
                named(value)
            ),
        });
        return;
    }

    match value {
        Toml::Table(table) => table_against(table, schema, root, at, found),
        Toml::Array(entries) => {
            if let Some(each) = schema.get("items") {
                for (index, entry) in entries.iter().enumerate() {
                    against(entry, each, root, &placed(at, index, entry), found);
                }
            }
        }
        Toml::Integer(number) => bounded(*number, schema, at, found),
        _ => {}
    }
}

/// One table against the fields the schema declares for it.
///
/// Three questions rather than one, because they are three different mistakes: a word
/// this build has no field for, a field it needs and did not get, and a field whose
/// value is the wrong kind of thing. An author is told all of them at once.
fn table_against(
    table: &toml::map::Map<String, Toml>,
    schema: &Json,
    root: &Json,
    at: &str,
    found: &mut Vec<Violation>,
) {
    let Some(properties) = schema.get("properties").and_then(Json::as_object) else {
        // A map rather than a record: every key is the author's, and the schema
        // constrains the values alone.
        if let Some(each) = schema
            .get("additionalProperties")
            .filter(|it| it.is_object())
        {
            for (key, value) in table {
                against(value, each, root, &under(at, key), found);
            }
        }
        return;
    };

    let closed = schema.get("additionalProperties") == Some(&Json::Bool(false));
    if closed {
        let takes = listed(properties.keys().cloned());
        for key in table.keys().filter(|key| !properties.contains_key(*key)) {
            found.push(Violation {
                location: under(at, key),
                message: format!(
                    "this build declares no such field here; what may be declared is: {takes}"
                ),
            });
        }
    }

    if let Some(required) = schema.get("required").and_then(Json::as_array) {
        for name in required.iter().filter_map(Json::as_str) {
            if !table.contains_key(name) {
                found.push(Violation {
                    location: under(at, name),
                    message: "is required and is not declared".to_owned(),
                });
            }
        }
    }

    for (key, value) in table {
        if let Some(each) = properties.get(key) {
            against(value, each, root, &under(at, key), found);
        }
    }
}

/// A value against a set of alternatives, of which it must satisfy one.
///
/// A closed set of words is refused as a word rather than as a failed alternative: the
/// author wrote a name, and being told their name is not one of four is the answer,
/// where being told it satisfied neither of four subschemas is a riddle. Anything else
/// reports whichever branch it came closest to, because a list of every way it is not
/// each of three shapes is longer than it is useful.
fn alternatives(
    value: &Toml,
    branches: &[Json],
    root: &Json,
    at: &str,
    found: &mut Vec<Violation>,
) {
    let mut nearest: Option<Vec<Violation>> = None;
    for branch in branches {
        let mut said = Vec::new();
        against(value, branch, root, at, &mut said);
        if said.is_empty() {
            return;
        }
        if nearest.as_ref().is_none_or(|best| said.len() < best.len()) {
            nearest = Some(said);
        }
    }

    let words: Vec<&Json> = branches
        .iter()
        .filter_map(|branch| resolved(branch, root).get("const"))
        .collect();
    if words.len() == branches.len() && !words.is_empty() {
        found.push(Violation {
            location: at.to_owned(),
            message: format!(
                "{} is not one of: {}",
                said(value),
                listed(words.iter().map(|word| rendered(word)))
            ),
        });
        return;
    }

    // Nothing to report only where there were no branches to try, which a generated
    // schema does not produce; the empty case is still an empty answer rather than a
    // silent pass, because `nearest` holding nothing means nothing was asked.
    if let Some(best) = nearest {
        found.extend(best);
    } else {
        found.push(Violation {
            location: at.to_owned(),
            message: "the schema offers no shape this could take".to_owned(),
        });
    }
}

/// A whole number against the range the schema permits.
fn bounded(number: i64, schema: &Json, at: &str, found: &mut Vec<Violation>) {
    if let Some(least) = schema.get("minimum").and_then(Json::as_i64) {
        if number < least {
            found.push(Violation {
                location: at.to_owned(),
                message: format!("{number} is below the least this may be, {least}"),
            });
        }
    }
    if let Some(most) = schema.get("maximum").and_then(Json::as_i64) {
        if number > most {
            found.push(Violation {
                location: at.to_owned(),
                message: format!("{number} is above the most this may be, {most}"),
            });
        }
    }
}

/// What a `$ref` points at, or the schema itself where it points at nothing.
fn resolved<'a>(schema: &'a Json, root: &'a Json) -> &'a Json {
    schema
        .get("$ref")
        .and_then(Json::as_str)
        .and_then(|pointer| pointer.strip_prefix('#'))
        .and_then(|path| root.pointer(path))
        .unwrap_or(schema)
}

/// The alternatives a schema offers, under either of the two words for them.
fn branches(schema: &Json) -> Option<&[Json]> {
    schema
        .get("oneOf")
        .or_else(|| schema.get("anyOf"))
        .and_then(Json::as_array)
        .map(Vec::as_slice)
}

/// The kinds of thing a schema says a value may be.
fn kinds(schema: &Json) -> Vec<&str> {
    match schema.get("type") {
        Some(Json::String(one)) => vec![one.as_str()],
        Some(Json::Array(many)) => many.iter().filter_map(Json::as_str).collect(),
        _ => Vec::new(),
    }
}

/// Whether a value is of the kind a schema names.
///
/// `null` is never satisfied: TOML has no way to write one, so an optional field is
/// absent rather than empty, and a value that is present has to satisfy the other half.
fn holds(value: &Toml, kind: &str) -> bool {
    match kind {
        "string" => value.is_str(),
        "integer" => value.is_integer(),
        "number" => value.is_integer() || value.is_float(),
        "boolean" => value.is_bool(),
        "array" => value.is_array(),
        "object" => value.is_table(),
        _ => false,
    }
}

/// Whether a value is exactly the one the schema fixes.
fn is(value: &Toml, only: &Json) -> bool {
    match (value, only) {
        (Toml::String(wrote), Json::String(fixed)) => wrote == fixed,
        (Toml::Boolean(wrote), Json::Bool(fixed)) => wrote == fixed,
        (Toml::Integer(wrote), Json::Number(fixed)) => fixed.as_i64() == Some(*wrote),
        _ => false,
    }
}

/// A value as an author would recognise it, quoted where it is a word.
fn said(value: &Toml) -> String {
    match value {
        Toml::String(word) => format!("`{word}`"),
        Toml::Integer(number) => number.to_string(),
        Toml::Boolean(flag) => flag.to_string(),
        other => named(other).to_owned(),
    }
}

/// One of a closed set, as an author would type it.
fn rendered(only: &Json) -> String {
    only.as_str()
        .map_or_else(|| only.to_string(), |word| format!("`{word}`"))
}

/// What a value is, in the words TOML uses for it.
fn named(value: &Toml) -> &'static str {
    match value {
        Toml::String(_) => "a string",
        Toml::Integer(_) => "a whole number",
        Toml::Float(_) => "a number",
        Toml::Boolean(_) => "a true or a false",
        Toml::Datetime(_) => "a date",
        Toml::Array(_) => "a list",
        Toml::Table(_) => "a table",
    }
}

/// The kinds a schema will accept, in the words TOML uses for them.
fn wanted(kinds: &[&str]) -> String {
    listed(
        kinds
            .iter()
            .filter(|kind| **kind != "null")
            .map(|kind| match *kind {
                "string" => "a string",
                "integer" => "a whole number",
                "number" => "a number",
                "boolean" => "a true or a false",
                "array" => "a list",
                "object" => "a table",
                other => other,
            })
            .map(str::to_owned),
    )
}

/// A field inside a location, spelled the way the manifest spells it.
fn under(at: &str, key: &str) -> String {
    if at.is_empty() {
        return key.to_owned();
    }
    format!("{at}.{key}")
}

/// One entry of a list, placed by the id it declares or by where it sits.
fn placed(at: &str, index: usize, entry: &Toml) -> String {
    entry
        .get("id")
        .and_then(Toml::as_str)
        .map_or_else(|| format!("{at} {index}"), |id| format!("{at} {id}"))
}

/// Names in one line, in a stable order and without repeats.
///
/// One item type rather than anything that reads as a string. A generic here is
/// compiled once per type it is asked with, which is two sets of counters over one
/// set of lines — and a line taken in one of them and missed in the other is a miss
/// the coverage summary counts and its own line list cannot show.
fn listed(names: impl Iterator<Item = String>) -> String {
    names
        .collect::<BTreeSet<String>>()
        .into_iter()
        .fold(String::new(), |mut line, name| {
            if !line.is_empty() {
                line.push_str(", ");
            }
            line.push_str(&name);
            line
        })
}

#[cfg(test)]
mod tests;
