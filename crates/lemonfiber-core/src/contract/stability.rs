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
        fields_kept(name, was, now, found);
    }
}

/// Every field one type described, still described and still the same shape.
fn fields_kept(name: &str, was: &Shape, now: &Shape, found: &mut Vec<Break>) {
    for (field, before) in &was.fields {
        let Some(after) = now.fields.get(field) else {
            found.push(Break::new(
                format!("{name}.{field}"),
                "this field is gone, and a consumer reading it finds nothing there",
            ));
            continue;
        };
        for lost in before.types.difference(&after.types) {
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
    let schemas = described.get("kinds").and_then(Value::as_object);
    for (kind, schema) in schemas.into_iter().flatten() {
        let carried = schema
            .pointer("/properties/data")
            .map_or_else(|| ANY.to_owned(), token);
        kinds.insert(kind.clone(), carried);

        let named = schema.get("$defs").and_then(Value::as_object);
        for (name, shape) in named.into_iter().flatten() {
            gather(shape, types.entry(name.clone()).or_default());
        }
    }
    Surface {
        api_version,
        kinds,
        types,
    }
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
mod tests {
    use super::{gather, read, rendered, token, Break, Field, Shape, Surface, SURFACE_PATH};
    use crate::contract::Contract;
    use serde_json::{json, Value};
    use std::collections::{BTreeMap, BTreeSet};

    /// The committed surface, read from the workspace root.
    fn committed() -> Option<Surface> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let text = std::fs::read_to_string(root.join(SURFACE_PATH)).ok()?;
        Surface::parse(&text)
    }

    /// A surface holding one type with one required string field.
    fn one_field(name: &str, spelled: &str, required: bool) -> Surface {
        let mut types = BTreeMap::new();
        let mut fields = BTreeMap::new();
        fields.insert(
            name.to_owned(),
            Field {
                types: [spelled.to_owned()].into_iter().collect(),
                required,
            },
        );
        types.insert(
            "Report".to_owned(),
            Shape {
                fields,
                variants: BTreeSet::new(),
            },
        );
        Surface {
            api_version: 1,
            kinds: BTreeMap::new(),
            types,
        }
    }

    /// A surface describing one kind carrying one type.
    fn one_kind(kind: &str, carries: &str) -> Surface {
        let mut kinds = BTreeMap::new();
        kinds.insert(kind.to_owned(), carries.to_owned());
        Surface {
            api_version: 1,
            kinds,
            types: BTreeMap::new(),
        }
    }

    /// What a walk of one schema found.
    fn walked(schema: &Value) -> Shape {
        let mut shape = Shape::default();
        gather(schema, &mut shape);
        shape
    }

    #[test]
    fn a_plain_type_is_its_own_token() {
        assert_eq!(token(&json!({"type": "string"})), "string");
        assert_eq!(
            token(&json!({"type": "integer", "format": "uint32"})),
            "integer/uint32"
        );
    }

    #[test]
    fn what_a_value_means_is_left_out_of_its_token() {
        // The whole point of a second artefact: rewording a doc comment moves the
        // contract and must not move this.
        let described = json!({
            "type": "string",
            "description": "What this field is for, at length.",
            "title": "Something",
            "default": "nothing",
            "examples": ["one"]
        });
        assert_eq!(token(&described), "string");
    }

    #[test]
    fn a_nullable_value_reads_the_same_whichever_order_it_was_written_in() {
        assert_eq!(token(&json!({"type": ["string", "null"]})), "null|string");
        assert_eq!(token(&json!({"type": ["null", "string"]})), "null|string");
    }

    #[test]
    fn a_reference_is_named_by_the_type_it_points_at() {
        assert_eq!(token(&json!({"$ref": "#/$defs/Remedy"})), "ref:Remedy");
    }

    #[test]
    fn a_constant_is_the_value_it_is() {
        assert_eq!(token(&json!({"const": "wired"})), "const:\"wired\"");
    }

    #[test]
    fn a_list_and_a_map_carry_what_they_hold() {
        assert_eq!(
            token(&json!({"type": "array", "items": {"$ref": "#/$defs/Entry"}})),
            "array[ref:Entry]"
        );
        assert_eq!(
            token(&json!({"type": "object", "additionalProperties": {"type": "string"}})),
            "object{string}"
        );
        // A map that names no value type says nothing about one, which is not the
        // same as saying it holds anything.
        assert_eq!(
            token(&json!({"type": "object", "additionalProperties": false})),
            "object"
        );
    }

    #[test]
    fn a_union_reads_the_same_however_its_options_were_ordered() {
        let one = json!({"oneOf": [{"type": "null"}, {"$ref": "#/$defs/Plan"}]});
        let other = json!({"oneOf": [{"$ref": "#/$defs/Plan"}, {"type": "null"}]});
        assert_eq!(token(&one), token(&other));
        assert_eq!(token(&one), "oneOf(null,ref:Plan)");
        assert_eq!(
            token(&json!({"anyOf": [{"type": "string"}, {"type": "string"}]})),
            "anyOf(string)"
        );
    }

    #[test]
    fn a_schema_that_constrains_nothing_says_so() {
        assert_eq!(token(&json!({})), "any");
        assert_eq!(token(&json!("not a schema at all")), "any");
    }

    #[test]
    fn every_field_of_a_described_type_is_collected_with_whether_it_is_required() {
        let shape = walked(&json!({
            "type": "object",
            "properties": {
                "binary": {"type": "string"},
                "compose": {"type": ["string", "null"]}
            },
            "required": ["binary"]
        }));

        assert!(shape.fields.get("binary").is_some_and(|one| one.required));
        assert!(shape.fields.get("compose").is_some_and(|one| !one.required));
    }

    #[test]
    fn a_field_described_twice_keeps_both_spellings_and_the_weaker_demand() {
        // A tagged union describes `reason` once per variant, and one of them may
        // leave it out. A consumer has to code against the weaker of the two.
        let shape = walked(&json!({
            "oneOf": [
                {
                    "type": "object",
                    "properties": {"reason": {"type": "string"}},
                    "required": ["reason"]
                },
                {
                    "type": "object",
                    "properties": {"reason": {"type": ["string", "null"]}}
                }
            ]
        }));

        let reason = shape.fields.get("reason");
        assert!(reason.is_some_and(|one| one.types.len() == 2), "{shape:?}");
        assert!(reason.is_some_and(|one| !one.required), "{shape:?}");
    }

    #[test]
    fn every_constant_a_type_may_be_is_collected_however_it_is_written() {
        // A bare string enumeration, and one that tags its variants beside fields.
        let bare = walked(&json!({
            "oneOf": [
                {"type": "string", "const": "current"},
                {"type": "string", "const": "stale"}
            ]
        }));
        assert_eq!(bare.variants.len(), 2, "{bare:?}");
        assert!(bare.variants.contains("\"current\""), "{bare:?}");

        let tagged = walked(&json!({
            "oneOf": [{
                "type": "object",
                "properties": {"state": {"const": "wired"}},
                "required": ["state"]
            }]
        }));
        assert!(tagged.variants.contains("\"wired\""), "{tagged:?}");
    }

    #[test]
    fn a_field_named_after_a_schema_keyword_is_read_as_a_field() {
        // `properties` holds field names, so a field called `const` must not be read
        // as the keyword that says what a value may be.
        let shape = walked(&json!({
            "type": "object",
            "properties": {"const": {"type": "string"}},
            "required": ["const"]
        }));

        assert!(shape.variants.is_empty(), "{shape:?}");
        assert!(shape.fields.contains_key("const"), "{shape:?}");
    }

    #[test]
    fn a_keyword_holding_something_that_is_not_a_schema_is_walked_past() {
        let shape = walked(&json!({"type": "object", "additionalProperties": false}));
        assert_eq!(shape, Shape::default());
    }

    #[test]
    fn a_kind_is_recorded_by_the_payload_it_carries_and_its_types_by_name() {
        let described = json!({
            "kinds": {
                "version": {
                    "properties": {"data": {"$ref": "#/$defs/VersionReport"}},
                    "$defs": {
                        "VersionReport": {
                            "type": "object",
                            "properties": {"binary": {"type": "string"}},
                            "required": ["binary"]
                        }
                    }
                }
            }
        });
        let surface = read(&described, 1);

        assert_eq!(
            surface.kinds.get("version").map(String::as_str),
            Some("ref:VersionReport")
        );
        assert!(surface.types.contains_key("VersionReport"), "{surface:?}");
    }

    #[test]
    fn a_kind_carrying_no_payload_is_recorded_as_carrying_nothing_in_particular() {
        let surface = read(&json!({"kinds": {"start": {}}}), 1);
        assert_eq!(surface.kinds.get("start").map(String::as_str), Some("any"));
    }

    #[test]
    fn nothing_described_at_all_is_an_empty_surface_rather_than_a_failure() {
        assert_eq!(read(&json!({}), 1).kinds.len(), 0);
    }

    #[test]
    fn a_type_carried_by_two_kinds_is_recorded_once() {
        let shape = json!({
            "type": "object",
            "properties": {"detail": {"type": "string"}},
            "required": ["detail"]
        });
        let described = json!({
            "kinds": {
                "one": {"$defs": {"Problem": shape.clone()}},
                "two": {"$defs": {"Problem": shape}}
            }
        });

        let surface = read(&described, 1);
        assert_eq!(surface.types.len(), 1, "{surface:?}");
    }

    #[test]
    fn two_surfaces_that_agree_break_nothing() {
        let surface = one_field("binary", "string", true);
        assert!(Surface::broken(&surface, &surface).is_empty());
    }

    #[test]
    fn a_field_that_is_added_is_not_a_break() {
        let before = one_field("binary", "string", true);
        let mut after = before.clone();
        if let Some(shape) = after.types.get_mut("Report") {
            shape.fields.insert("stack".to_owned(), Field::default());
        }

        assert!(Surface::broken(&before, &after).is_empty());
    }

    #[test]
    fn a_field_that_is_gone_is_named() {
        let before = one_field("binary", "string", true);
        let after = one_field("stack", "string", true);

        let broken = Surface::broken(&before, &after);
        assert_eq!(
            broken.first().map(|one| one.what.clone()),
            Some("Report.binary".to_owned()),
            "{broken:?}"
        );
    }

    #[test]
    fn a_field_that_changed_type_is_named_with_the_type_it_had() {
        let before = one_field("supported", "array[integer/uint32]", true);
        let after = one_field("supported", "string", true);

        let broken = Surface::broken(&before, &after);
        assert!(
            broken
                .first()
                .is_some_and(|one| one.because.contains("array[integer/uint32]")),
            "{broken:?}"
        );
    }

    #[test]
    fn a_field_that_was_always_there_and_may_now_be_absent_is_named() {
        let before = one_field("binary", "string", true);
        let after = one_field("binary", "string", false);

        let broken = Surface::broken(&before, &after);
        assert!(
            broken
                .first()
                .is_some_and(|one| one.because.contains("absent")),
            "{broken:?}"
        );
        // And the other way round is nothing: a field that may now always be there
        // is one a consumer has already coded a check for.
        assert!(Surface::broken(&after, &before).is_empty());
    }

    #[test]
    fn a_type_that_is_gone_is_named_rather_than_its_fields_one_by_one() {
        let before = one_field("binary", "string", true);
        let after = Surface {
            api_version: 1,
            ..Surface::default()
        };

        let broken = Surface::broken(&before, &after);
        assert_eq!(broken.len(), 1, "{broken:?}");
        assert_eq!(
            broken.first().map(|one| one.what.clone()),
            Some("Report".to_owned())
        );
    }

    #[test]
    fn a_value_taken_off_the_wire_is_named_beside_the_type_that_held_it() {
        let mut before = one_field("state", "string", true);
        if let Some(shape) = before.types.get_mut("Report") {
            shape.variants.insert("\"wired\"".to_owned());
        }
        let after = one_field("state", "string", true);

        let broken = Surface::broken(&before, &after);
        assert_eq!(
            broken.first().map(|one| one.what.clone()),
            Some("Report = \"wired\"".to_owned()),
            "{broken:?}"
        );
        // Added the other way round, which is a consumer's to ignore.
        assert!(Surface::broken(&after, &before).is_empty());
    }

    #[test]
    fn a_kind_that_is_no_longer_emitted_is_named() {
        let before = one_kind("version", "ref:VersionReport");
        let after = Surface {
            api_version: 1,
            ..Surface::default()
        };

        let broken = Surface::broken(&before, &after);
        assert!(
            broken.first().is_some_and(|one| one.what == "version"),
            "{broken:?}"
        );
    }

    #[test]
    fn a_kind_whose_payload_changed_is_named_with_both_shapes() {
        let before = one_kind("version", "ref:VersionReport");
        let after = one_kind("version", "ref:Something");

        let broken = Surface::broken(&before, &after);
        assert!(
            broken
                .first()
                .is_some_and(|one| one.because.contains("ref:VersionReport")
                    && one.because.contains("ref:Something")),
            "{broken:?}"
        );
    }

    #[test]
    fn a_kind_that_is_added_is_not_a_break() {
        let before = Surface {
            api_version: 1,
            ..Surface::default()
        };
        let after = one_kind("version", "ref:VersionReport");

        assert!(Surface::broken(&before, &after).is_empty());
    }

    #[test]
    fn moving_the_wire_version_is_what_makes_a_removal_allowed() {
        let before = one_field("binary", "string", true);
        let mut after = Surface {
            api_version: 2,
            ..Surface::default()
        };
        assert!(Surface::broken(&before, &after).is_empty());

        // And it is the version that did it, not the emptiness: at the same version
        // the same pair is a break.
        after.api_version = 1;
        assert!(!Surface::broken(&before, &after).is_empty());
    }

    #[test]
    fn every_break_is_rendered_with_what_moved_and_why_it_matters() {
        let rendering = rendered(&[Break::new("Report.binary".to_owned(), "it is gone")]);
        assert_eq!(rendering, "  Report.binary — it is gone\n");
        assert_eq!(rendered(&[]), String::new());
    }

    /// The promise the version number carries, held against what this build describes.
    ///
    /// The half that the comparison against the types cannot make: that one tells a
    /// stale artefact from a current one, and both a removal and an addition are
    /// merely stale to it. This one is only ever about what went missing.
    #[test]
    fn nothing_the_released_surface_describes_is_removed_or_retyped_under_one_version() {
        let before = committed();
        assert!(
            before.is_some(),
            "there is no committed surface to hold this build to — write one with \
             `just surface`, which refuses to write a surface that drops anything"
        );

        let broken = Surface::broken(
            &before.unwrap_or_default(),
            &Surface::of(&Contract::describe()),
        );
        let named = rendered(&broken);

        assert!(
            broken.is_empty(),
            "these are promises the machine-readable output has made and this build no longer \
             keeps, under an unchanged api_version:\n{named}\nEither put them back, or \
             increment `API_VERSION` and rewrite the surface with `just surface`."
        );
    }

    /// And the surface is kept current, so what it holds this build to is everything
    /// this build has published rather than everything it published once.
    ///
    /// Without this a field added after the surface was last written could be taken
    /// out again with nothing to say so — it was never in the surface to be missed.
    #[test]
    fn the_committed_surface_still_describes_what_these_types_do() {
        let fresh = Surface::of(&Contract::describe());
        let stored = committed().unwrap_or_default();

        assert_eq!(
            stored, fresh,
            "the surface is out of date — rewrite it with `just surface`"
        );
    }

    #[test]
    fn a_surface_round_trips_through_the_form_it_is_committed_in() {
        let surface = one_field("binary", "string", true);
        let written = surface.to_json().unwrap_or_default();

        assert!(written.ends_with("}\n"), "{written}");
        assert_eq!(Surface::parse(&written), Some(surface));
        // And anything that is not one reads as nothing to compare against, rather
        // than as an empty surface that would silently pass every comparison.
        assert_eq!(Surface::parse("not a surface at all"), None);
    }
}
