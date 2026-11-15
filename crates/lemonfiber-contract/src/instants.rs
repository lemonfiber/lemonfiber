//! Where an answer holds an instant: each place the schema of what is answered types as
//! one, found by walking the answer and its schema together.

use schemars::JsonSchema;
use serde_json::{Map, Value};

use lemonfiber_ports::Instant;

/// The members of a schema that annotate or carry it rather than say what it admits.
const ANNOTATING: [&str; 3] = ["$schema", "title", "$defs"];

/// How many references a walk follows at one place of the answer.
const REFERRED_MOST: usize = 32;

/// Each place `answer` holds an instant `schema` types as one, as a JSON pointer, in order
/// and once.
#[must_use]
pub fn held(schema: &Value, answer: &Value) -> Vec<String> {
    let instant = definition();
    let mut walk = Walk {
        root: schema,
        instant: &instant,
        found: Vec::new(),
    };
    walk.at(schema, answer, "", 0);
    walk.found.sort();
    walk.found.dedup();
    walk.found
}

/// The schema an instant is defined by.
fn definition() -> Map<String, Value> {
    let mut generator = schemars::SchemaGenerator::default();
    let _ = generator.subschema_for::<Instant>();
    generator
        .definitions()
        .get(Instant::schema_name().as_ref())
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

/// One walk of an answer beside the schema rooted at `root`.
struct Walk<'s> {
    root: &'s Value,
    instant: &'s Map<String, Value>,
    found: Vec<String>,
}

impl<'s> Walk<'s> {
    /// Walk `value` at `place` beside `schema`, having followed `referred` references there.
    fn at(&mut self, schema: &'s Value, value: &Value, place: &str, referred: usize) {
        let Some(schema) = schema.as_object() else {
            return;
        };
        if self.is_instant(schema) {
            if value.is_u64() {
                self.found.push(place.to_owned());
            }
            return;
        }
        if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
            if let Some(resolved) = self.resolved(reference, referred) {
                self.at(resolved, value, place, referred + 1);
            }
        }
        for (keyword, every) in [("allOf", true), ("anyOf", false), ("oneOf", false)] {
            for branch in schema
                .get(keyword)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if every || self.admits(branch, value, referred) {
                    self.at(branch, value, place, referred);
                }
            }
        }
        match value {
            Value::Object(members) => self.members(schema, members, place),
            Value::Array(entries) => {
                let prefix = schema.get("prefixItems").and_then(Value::as_array);
                for (index, entry) in entries.iter().enumerate() {
                    let sub = prefix
                        .and_then(|prefix| prefix.get(index))
                        .or_else(|| schema.get("items"));
                    if let Some(sub) = sub {
                        self.at(sub, entry, &format!("{place}/{index}"), 0);
                    }
                }
            }
            _ => {}
        }
    }

    /// Walk each member of `members` beside the schema `schema` names it with.
    fn members(
        &mut self,
        schema: &'s Map<String, Value>,
        members: &Map<String, Value>,
        place: &str,
    ) {
        let named = schema.get("properties").and_then(Value::as_object);
        let extra = schema
            .get("additionalProperties")
            .filter(|extra| extra.is_object());
        for (name, member) in members {
            let sub = named.and_then(|named| named.get(name)).or(extra);
            if let Some(sub) = sub {
                let step = name.replace('~', "~0").replace('/', "~1");
                self.at(sub, member, &format!("{place}/{step}"), 0);
            }
        }
    }

    /// Whether `schema` is an instant's, whatever annotates it.
    fn is_instant(&self, schema: &Map<String, Value>) -> bool {
        let mut admitting = schema
            .iter()
            .filter(|(name, _)| !ANNOTATING.contains(&name.as_str()));
        admitting.clone().count() == self.instant.len()
            && admitting.all(|(name, said)| self.instant.get(name) == Some(said))
    }

    /// The definition `reference` points to, where it is one of the root's and fewer than
    /// [`REFERRED_MOST`] were followed to reach it.
    fn resolved(&self, reference: &str, referred: usize) -> Option<&'s Value> {
        (referred < REFERRED_MOST)
            .then(|| reference.strip_prefix('#'))
            .flatten()
            .and_then(|pointer| self.root.pointer(pointer))
    }

    /// Whether `branch` of an `anyOf` or `oneOf` may be the one `value` is: its type, a
    /// constant or a member it requires does not rule it out.
    fn admits(&self, branch: &'s Value, value: &Value, referred: usize) -> bool {
        let mut branch = branch;
        let mut followed = referred;
        while let Some(reference) = branch.get("$ref").and_then(Value::as_str) {
            match self.resolved(reference, followed) {
                Some(resolved) => branch = resolved,
                None => return false,
            }
            followed += 1;
        }
        let Some(branch) = branch.as_object() else {
            return false;
        };
        if self.is_instant(branch) {
            return value.is_u64();
        }
        typed(branch.get("type"), value)
            && branch.get("const").is_none_or(|constant| constant == value)
            && branch
                .get("enum")
                .and_then(Value::as_array)
                .is_none_or(|listed| listed.contains(value))
            && named_alike(branch, value)
    }
}

/// Whether `value` is of a type `types` names, or `types` names none.
fn typed(types: Option<&Value>, value: &Value) -> bool {
    let named = |name: &str| match value {
        Value::Null => name == "null",
        Value::Bool(_) => name == "boolean",
        Value::Number(number) => name == "number" || (name == "integer" && !number.is_f64()),
        Value::String(_) => name == "string",
        Value::Array(_) => name == "array",
        Value::Object(_) => name == "object",
    };
    match types {
        None => true,
        Some(Value::String(name)) => named(name),
        Some(Value::Array(listed)) => listed.iter().filter_map(Value::as_str).any(named),
        Some(_) => false,
    }
}

/// Whether an object `value` holds every member `branch` requires, and each member
/// `branch` holds constant at that constant.
fn named_alike(branch: &Map<String, Value>, value: &Value) -> bool {
    let Some(members) = value.as_object() else {
        return true;
    };
    let required = branch
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .all(|name| members.contains_key(name));
    let constant = branch
        .get("properties")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(name, sub)| Some((name, sub.get("const")?)))
        .all(|(name, constant)| members.get(name).is_none_or(|held| held == constant));
    required && constant
}

#[cfg(test)]
mod tests;
