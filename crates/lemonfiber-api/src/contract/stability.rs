//! Whether the contract still describes everything the released one described.
//!
//! The generated artefact beside this says what the surfaces exchange *now*. That
//! it regenerates without a diff proves it is not stale; it proves nothing at all
//! about the promise the version number carries, which is that a field a script
//! reads goes on being there and goes on holding what it held. A removal and an
//! addition look the same to a comparison against the types, because both of them
//! are simply the artefact being out of date — and the failure says "regenerate
//! it" either way.
//!
//! So a second, much smaller artefact is kept: the *surface*. Names and types and
//! nothing else — no descriptions, no titles, no ordering — which is exactly the
//! part a consumer is entitled to rely on and none of the part that moves when
//! somebody rewrites a doc comment. Every change to the shapes is held against the
//! committed surface before it is allowed to replace it, and a field that went
//! missing or changed type under an unchanged version stops the build by name.
//!
//! # Why a committed surface rather than the previous released artefact
//!
//! Reaching for the last release would mean reading a git tag, a published file or
//! a second checkout from a test, and a guard that needs the network or the history
//! is a guard that is skipped on the machine where it matters. A committed file is
//! read the same way the artefact beside it already is, and its diff is the review:
//! an addition shows as added lines, and a removal cannot show at all, because the
//! generator refuses to write one.
//!
//! That refusal is the half that makes this a ratchet rather than a suggestion.
//! Were the surface simply regenerated from the types, whoever removed a field
//! would regenerate it along with the artefact and the guard would compare a shape
//! with itself. [`Surface::broken`] is therefore asked by the generator as well as
//! by the test, so the only way to land a removal is to move the wire version —
//! which is the decision the rule was always about.
//!
//! # What counts as breaking, and what does not
//!
//! A consumer parses what arrives. Something new arriving is theirs to ignore;
//! something they read failing to arrive is not. So an added kind, an added type,
//! an added field and an added enum variant are all additive and pass, while a
//! removal of any of them, a field whose type moved, and a field that was always
//! present becoming optional are refused. A field's type is compared as a token
//! rather than as the schema node it came from, so re-wording the sentence above a
//! field leaves this artefact alone.
//!
//! One narrowing is recognised as the narrowing it is. A field described as a bare
//! `string` that comes to point at a named closed set of strings still delivers a
//! string, and every value it can now carry is one it could carry before — so a
//! consumer parsing it the old way parses every value it will ever receive. What is
//! recognised is only that: a bare string becoming a reference to a definition that
//! is nothing *but* string constants. A reference is otherwise compared by its name,
//! because resolving references in general would let a field swap one object type
//! for another and pass. And the set it narrows to is a described type in its own
//! right, so a value later taken out of it is refused like any other.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::Contract;

/// Where the committed surface is kept, relative to the workspace root.
pub const SURFACE_PATH: &str = "contract/web-api.surface.json";

/// The token for a schema that constrains nothing — the shape of `data` for a kind
/// that carries no payload, and of a schema keyword this reads no further into.
const ANY: &str = "any";

/// The schema keywords whose value is itself a schema, or a list of them.
///
/// `properties` is not among them and is walked separately: its keys are field names
/// rather than keywords, so a struct with a field called `const` or `items` would be
/// read as a schema keyword by anything that treated the two alike.
const DESCENDED: [&str; 6] = [
    "items",
    "additionalProperties",
    "oneOf",
    "anyOf",
    "allOf",
    "prefixItems",
];

/// One field of one described type, as a consumer meets it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
    /// Every shape this name is described with.
    ///
    /// A set rather than one token because a tagged union describes the same field
    /// once per variant, and two variants may spell it differently. Holding all of
    /// them is what lets a variant's spelling go missing and be noticed, where one
    /// token would have recorded whichever variant was written last.
    #[serde(rename = "type")]
    pub types: BTreeSet<String>,
    /// Whether every description of this field requires it.
    ///
    /// False where any of them makes it optional, so a field that is required in
    /// one variant and absent in another is recorded as the weaker of the two —
    /// which is what a consumer has to code against.
    pub required: bool,
}

/// One described type: its fields, and the constants it may be.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shape {
    /// The fields, by name.
    #[serde(default)]
    pub fields: BTreeMap<String, Field>,
    /// Every constant the type or its fields may hold — the variant names of an
    /// enumeration, whether it serialises as a bare string or as a tag beside its
    /// fields. Removing one takes a value off the wire that a consumer may be
    /// matching on, so they are held here rather than left to the field tokens.
    #[serde(default)]
    pub variants: BTreeSet<String>,
}

/// The part of the machine-readable contract a consumer is entitled to rely on.
///
/// Keyed by type name rather than by the kind carrying it, which the artefact's own
/// sweeps make safe: a definition name describes one shape wherever it is found, so
/// a type shared by nine kinds is recorded once and compared once.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Surface {
    /// The wire version these shapes belong to.
    pub api_version: u32,
    /// Each emitted kind, and the shape of the payload it carries.
    #[serde(default)]
    pub kinds: BTreeMap<String, String>,
    /// Every type the shapes are made of.
    #[serde(default)]
    pub types: BTreeMap<String, Shape>,
    /// The definitions that are nothing but string constants, read off their schemas.
    ///
    /// Held in memory and never committed. It is only ever asked of the surface a build
    /// describes now, which is always read from the schemas themselves; and it cannot be
    /// worked out from [`Shape`] alone, because a shape records the constants a type may
    /// be and not the alternatives beside them that are not constants at all.
    #[serde(skip)]
    pub strings: BTreeSet<String>,
}

/// A promise the contract made and no longer keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Break {
    /// What moved, named the way a consumer would address it.
    pub what: String,
    /// Why that is a change a consumer cannot absorb.
    pub because: String,
}

impl Break {
    /// One break, named and explained.
    fn new(what: String, because: &str) -> Self {
        Self {
            what,
            because: because.to_owned(),
        }
    }
}

impl Surface {
    /// The surface of the contract this build describes.
    #[must_use]
    pub fn of(contract: &Contract) -> Self {
        // A tree of schemas is plain data, so it cannot fail to become a value; an
        // empty one on the impossible branch keeps this free of a line no test can
        // reach, and would be caught immediately by the comparison below anyway.
        let described = serde_json::to_value(contract).unwrap_or_default();
        read(&described, contract.api_version)
    }

    /// As it is committed: sorted keys, two-space indent, one trailing newline —
    /// written the way the artefact beside it is, so the two diff alike.
    ///
    /// `None` only if it cannot serialise, which maps of strings cannot.
    #[must_use]
    pub fn to_json(&self) -> Option<String> {
        let mut text = serde_json::to_string_pretty(self).ok()?;
        text.push('\n');
        Some(text)
    }

    /// What was committed, or nothing where there is no readable surface to compare
    /// against.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        serde_json::from_str(text).ok()
    }

    /// Every promise `before` made that `after` does not keep.
    ///
    /// Empty where the wire version moved between the two: incrementing it is the
    /// declaration that this is a different interface, and holding a new version to
    /// the old one's shapes would leave the rule with no way to be obeyed. The
    /// committed surface has to be rewritten for the new version, which the
    /// comparison beside this one insists on.
    #[must_use]
    pub fn broken(before: &Self, after: &Self) -> Vec<Break> {
        if before.api_version != after.api_version {
            return Vec::new();
        }
        let mut found = Vec::new();
        kinds_kept(before, after, &mut found);
        types_kept(before, after, &mut found);
        found
    }
}

/// Every kind `before` emitted, still emitted and still carrying the same payload.
fn kinds_kept(before: &Surface, after: &Surface, found: &mut Vec<Break>) {
    for (kind, was) in &before.kinds {
        match after.kinds.get(kind) {
            None => found.push(Break::new(
                kind.clone(),
                "this kind is no longer emitted, so a consumer branching on it has nothing left \
                 to parse",
            )),
            Some(now) if now != was => found.push(Break::new(
                kind.clone(),
                &format!("the payload this kind carries was {was} and is now {now}"),
            )),
            Some(_) => {}
        }
    }
}

/// Every type `before` described, still described and still holding what it held.
fn types_kept(before: &Surface, after: &Surface, found: &mut Vec<Break>) {
    for (name, was) in &before.types {
        let Some(now) = after.types.get(name) else {
            found.push(Break::new(
                name.clone(),
                "this type is no longer described, so anything generated from it has no \
                 definition to key by",
            ));
            continue;
        };
        for gone in was.variants.difference(&now.variants) {
            found.push(Break::new(
                format!("{name} = {gone}"),
                "this value is no longer on the wire, and a consumer matching on it has a case \
                 that can never be taken",
            ));
        }
        fields_kept(name, was, now, &after.strings, found);
    }
}

/// Every field one type described, still described and still the same shape.
fn fields_kept(
    name: &str,
    was: &Shape,
    now: &Shape,
    strings: &BTreeSet<String>,
    found: &mut Vec<Break>,
) {
    for (field, before) in &was.fields {
        let Some(after) = now.fields.get(field) else {
            found.push(Break::new(
                format!("{name}.{field}"),
                "this field is gone, and a consumer reading it finds nothing there",
            ));
            continue;
        };
        for lost in before.types.difference(&after.types) {
            if narrowed(lost, &after.types, strings) {
                continue;
            }
            found.push(Break::new(
                format!("{name}.{field}"),
                &format!(
                    "this field was described as {lost} and no longer is, so a consumer parsing \
                     it that way fails on the value it receives"
                ),
            ));
        }
        if before.required && !after.required {
            found.push(Break::new(
                format!("{name}.{field}"),
                "this field was always present and may now be absent, which a consumer that \
                 never had to check for it will read as missing data",
            ));
        }
    }
}

/// Whether a field that stopped being described as `lost` is still read that way.
///
/// True in the one case this recognises: `lost` is a bare string, and the field now
/// points at a definition whose schema admits string constants and nothing else. Every
/// value the field can carry is still a string, so nothing a consumer parses fails.
fn narrowed(lost: &str, now: &BTreeSet<String>, strings: &BTreeSet<String>) -> bool {
    lost == "string"
        && now
            .iter()
            .filter_map(|spelled| spelled.strip_prefix("ref:"))
            .any(|name| strings.contains(name))
}

/// Every break, as one block of prose to fail a build with.
///
/// Rendered here rather than inside an assertion, because an assertion's message is
/// evaluated only where it fails — and a rendering nothing runs while the guard
/// passes is a rendering nobody has watched work.
#[must_use]
pub fn rendered(breaks: &[Break]) -> String {
    let mut out = String::new();
    for one in breaks {
        let _ = writeln!(out, "  {} — {}", one.what, one.because);
    }
    out
}

/// The surface of an already-serialised contract.
///
/// Apart from [`Surface::of`] so that the walk can be driven with shapes written by
/// hand: every case it has to get right is a schema three lines long, and building
/// one through the report types that happen to have that shape would tie this to
/// whichever report currently does.
fn read(described: &Value, api_version: u32) -> Surface {
    let mut kinds = BTreeMap::new();
    let mut types: BTreeMap<String, Shape> = BTreeMap::new();
    let mut strings = BTreeSet::new();
    let schemas = described.get("kinds").and_then(Value::as_object);
    for (kind, schema) in schemas.into_iter().flatten() {
        let carried = schema
            .pointer("/properties/data")
            .map_or_else(|| ANY.to_owned(), token);
        kinds.insert(kind.clone(), carried);

        let named = schema.get("$defs").and_then(Value::as_object);
        for (name, shape) in named.into_iter().flatten() {
            gather(shape, types.entry(name.clone()).or_default());
            if only_string_constants(shape) {
                strings.insert(name.clone());
            }
        }
    }
    Surface {
        api_version,
        kinds,
        types,
        strings,
    }
}

/// Whether a definition's schema admits string constants and nothing else.
///
/// Read from the schema rather than from the shape gathered out of it, because the
/// shape keeps the constants and drops an alternative that is not one: a set of words
/// with a bare integer beside it gathers to the same shape as the words alone.
///
/// Two spellings are recognised, the two a generator writes a closed set of words in: a
/// string type with an `enum` of strings, and a choice every one of whose options is a
/// single string constant. Anything else is not a closed set of strings.
fn only_string_constants(schema: &Value) -> bool {
    let a_string = |node: &Value| node.get("type").is_none_or(|kind| kind == "string");
    if let Some(Value::Array(words)) = schema.get("enum") {
        return a_string(schema) && !words.is_empty() && words.iter().all(Value::is_string);
    }
    ["oneOf", "anyOf"].iter().any(|keyword| {
        schema
            .get(*keyword)
            .and_then(Value::as_array)
            .is_some_and(|options| {
                !options.is_empty()
                    && options.iter().all(|option| {
                        a_string(option)
                            && option.get("const").is_some_and(Value::is_string)
                            && option.get("properties").is_none()
                    })
            })
    })
}

/// One schema node as the token a consumer would code against.
///
/// Everything that says what a value *is* and nothing that says what it means: the
/// type, the format that narrows it, what an array holds, what a map maps to, and
/// the alternatives a union offers. A description, a title, a default and an example
/// are all absent by construction rather than by exclusion, which is what keeps this
/// artefact still while doc comments move.
fn token(node: &Value) -> String {
    let Some(fields) = node.as_object() else {
        return ANY.to_owned();
    };
    if let Some(reference) = fields.get("$ref").and_then(Value::as_str) {
        return format!("ref:{}", reference.trim_start_matches("#/$defs/"));
    }
    if let Some(constant) = fields.get("const") {
        return format!("const:{constant}");
    }
    let mut written = match fields.get("type") {
        Some(Value::String(one)) => one.clone(),
        // A nullable value is described as two types, and the pair is sorted so the
        // token does not depend on which the generator wrote first.
        Some(Value::Array(several)) => {
            let mut names: Vec<&str> = several.iter().filter_map(Value::as_str).collect();
            names.sort_unstable();
            names.join("|")
        }
        _ => String::new(),
    };
    if let Some(format) = fields.get("format").and_then(Value::as_str) {
        let _ = write!(written, "/{format}");
    }
    if let Some(items) = fields.get("items") {
        let _ = write!(written, "[{}]", token(items));
    }
    if let Some(values) = fields
        .get("additionalProperties")
        .filter(|extra| extra.is_object())
    {
        let _ = write!(written, "{{{}}}", token(values));
    }
    for keyword in ["oneOf", "anyOf"] {
        if let Some(Value::Array(options)) = fields.get(keyword) {
            let mut offered: Vec<String> = options.iter().map(token).collect();
            offered.sort();
            offered.dedup();
            let _ = write!(written, "{keyword}({})", offered.join(","));
        }
    }
    if written.is_empty() {
        return ANY.to_owned();
    }
    written
}

/// Walk one described type, collecting its fields and the constants it may be.
///
/// Everything a type holds lands under that type's own name, however deeply the
/// generator nested it: a tagged union's variants are objects inside the union's own
/// schema rather than definitions of their own, so a variant's fields belong to the
/// union as far as anything reading this is concerned.
fn gather(node: &Value, shape: &mut Shape) {
    match node {
        Value::Array(several) => {
            for one in several {
                gather(one, shape);
            }
        }
        Value::Object(fields) => {
            if let Some(constant) = fields.get("const") {
                shape.variants.insert(constant.to_string());
            }
            if let Some(Value::Object(properties)) = fields.get("properties") {
                let required: BTreeSet<&str> = fields
                    .get("required")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect();
                for (name, sub) in properties {
                    let field = shape.fields.entry(name.clone()).or_insert(Field {
                        types: BTreeSet::new(),
                        required: true,
                    });
                    field.types.insert(token(sub));
                    field.required &= required.contains(name.as_str());
                    gather(sub, shape);
                }
            }
            for keyword in DESCENDED {
                if let Some(sub) = fields.get(keyword) {
                    gather(sub, shape);
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests;
