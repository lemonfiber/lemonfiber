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
mod tests {
    use std::collections::BTreeSet;

    use serde_json::Value as Json;
    use toml::Value as Toml;

    use super::{
        against, is, listed, named, nonconforming, published, rendered, said as spoken, wanted,
        Violation,
    };

    /// What each refusal said, as one line per fault.
    fn said(text: &str) -> Vec<String> {
        nonconforming(text)
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    /// Whether some refusal carries every one of these words.
    fn names(said: &[String], words: &[&str]) -> bool {
        said.iter()
            .any(|one| words.iter().all(|word| one.contains(word)))
    }

    #[test]
    fn a_violation_names_where_it_is_before_what_it_is() {
        let one = Violation {
            location: "service komga".to_owned(),
            message: "bind: unknown".to_owned(),
        };
        assert_eq!(one.to_string(), "service komga: bind: unknown");
    }

    #[test]
    fn a_field_this_build_does_not_declare_is_named_with_those_it_does() {
        let said = said(
            "
[[service]]
id = \"komga\"
environment = { PUID = \"1000\" }
",
        );
        assert!(
            names(&said, &["service komga.environment", "no such field"]),
            "got: {said:?}"
        );
        assert!(
            names(&said, &["config_path", "criticality"]),
            "the fields it may declare are listed: {said:?}"
        );
    }

    #[test]
    fn a_required_field_that_is_absent_is_named() {
        let said = said(
            "
schema_version = 1

[[service]]
id = \"komga\"
",
        );
        assert!(
            names(&said, &["service komga.digest", "required"]),
            "got: {said:?}"
        );
        assert!(
            names(&said, &["plugin", "required"]),
            "a whole table that is absent is named too: {said:?}"
        );
    }

    #[test]
    fn a_value_of_the_wrong_kind_names_both_kinds() {
        let said = said(
            "
[[service]]
id   = \"komga\"
port = \"25600\"
",
        );
        assert!(
            names(&said, &["service komga.port", "whole number", "a string"]),
            "got: {said:?}"
        );
    }

    #[test]
    fn a_word_outside_a_closed_set_is_named_with_the_set() {
        let said = said(
            "
[[service]]
id   = \"komga\"
bind = \"wan\"
",
        );
        assert!(
            names(&said, &["service komga.bind", "`wan`"]),
            "got: {said:?}"
        );
        for available in ["loopback", "lan"] {
            assert!(names(&said, &[available]), "names {available}: {said:?}");
        }
    }

    /// The severity a plugin may not assign itself is refused as a word.
    ///
    /// It means *its failure has consequences outside the machine*, which is not a
    /// judgement a contributor makes about their own work. The four that are available
    /// come from the type, so nothing here holds a second copy of the list.
    #[test]
    fn the_criticality_a_plugin_may_not_claim_is_named_with_the_four_it_may() {
        let said = said(
            "
[[service]]
id          = \"komga\"
criticality = \"critical\"
",
        );
        assert!(names(&said, &["`critical`"]), "names the value: {said:?}");
        for available in ["core", "important", "enhancing", "optional"] {
            assert!(names(&said, &[available]), "names {available}: {said:?}");
        }
    }

    #[test]
    fn an_entry_with_no_id_yet_is_still_placed_by_where_it_sits() {
        let said = said(
            "
[[service]]
health = { kind = \"carrier-pigeon\" }
",
        );
        assert!(names(&said, &["service 0"]), "got: {said:?}");
    }

    /// Every fault at once, which is the whole reason this runs before the typed read.
    #[test]
    fn every_fault_in_one_pass_rather_than_the_first_one_reached() {
        let said = said(
            "
schema_version = 1

[[service]]
id          = \"komga\"
bind        = \"wan\"
port        = \"25600\"
environment = { PUID = \"1000\" }
",
        );
        assert!(names(&said, &["bind", "`wan`"]), "the word: {said:?}");
        assert!(
            names(&said, &["port", "whole number"]),
            "the kind: {said:?}"
        );
        assert!(
            names(&said, &["environment", "no such field"]),
            "the field: {said:?}"
        );
        assert!(
            names(&said, &["digest", "required"]),
            "and what is missing: {said:?}"
        );
    }

    #[test]
    fn a_nested_entry_is_placed_under_the_one_holding_it() {
        let said = said(
            "
[[recipe]]
id    = \"adopt\"
title = \"t\"
why   = \"w\"

[[recipe.step]]
id   = \"create\"
call = { method = \"POST\", to = \"komga\", path = \"/x\", nonsense = 1 }
",
        );
        assert!(
            names(&said, &["recipe adopt.step create.call.nonsense"]),
            "got: {said:?}"
        );
    }

    /// A map's keys are the author's own, so only its values are held to a shape.
    #[test]
    fn a_header_name_is_the_author_s_own_and_its_value_is_not() {
        let said = said(
            "
[[recipe]]
id    = \"adopt\"
title = \"t\"
why   = \"w\"

[[recipe.step]]
id   = \"create\"
call = { method = \"POST\", to = \"k\", path = \"/x\", headers = { Authorization = 1 } }
",
        );
        assert!(
            names(&said, &["headers.Authorization", "a string"]),
            "got: {said:?}"
        );
    }

    #[test]
    fn a_file_that_is_not_toml_is_left_to_the_reader() {
        assert!(nonconforming("= not toml").is_empty());
    }

    /// The walk is told to hold a shape the generated schema does not yet write.
    ///
    /// A generated schema is a thing that changes with the types behind it, and the
    /// walk answers about the whole of JSON Schema's small vocabulary rather than
    /// about the subset in use today. Driven directly, because reaching these through
    /// a manifest would mean a type this crate has no reason to declare.
    #[test]
    fn the_walk_answers_about_every_shape_a_generated_schema_can_take() {
        let held = |value: Toml, schema: &str| {
            let schema: Json = serde_json::from_str(schema).unwrap_or(Json::Null);
            let mut found = Vec::new();
            against(&value, &schema, &schema, "at", &mut found);
            found
        };

        // A number, which a float satisfies and a word does not.
        assert!(held(Toml::Float(1.5), r#"{"type":"number"}"#).is_empty());
        let said = held(Toml::String("x".to_owned()), r#"{"type":"number"}"#);
        assert!(
            said.first()
                .is_some_and(|one| one.message.contains("a number")),
            "got: {said:?}"
        );

        // A range, at both ends.
        let range = r#"{"type":"integer","minimum":1,"maximum":9}"#;
        assert!(held(Toml::Integer(5), range).is_empty());
        assert!(held(Toml::Integer(0), range)
            .first()
            .is_some_and(|one| one.message.contains("below")));
        assert!(held(Toml::Integer(99), range)
            .first()
            .is_some_and(|one| one.message.contains("above")));

        // A fixed flag and a fixed number, which are constants like a fixed word.
        assert!(held(Toml::Boolean(true), r#"{"const":true}"#).is_empty());
        assert!(held(Toml::Integer(2), r#"{"const":2}"#).is_empty());
        assert!(!held(Toml::Boolean(false), r#"{"const":true}"#).is_empty());
        assert!(!held(Toml::Integer(3), r#"{"const":2}"#).is_empty());

        // A schema saying nothing about a value accepts it.
        assert!(held(Toml::Float(2.5), r#"{"description":"anything"}"#).is_empty());

        // And a list of alternatives with none in it has nothing to offer.
        let said = held(Toml::Integer(1), r#"{"anyOf":[]}"#);
        assert!(
            said.first()
                .is_some_and(|one| one.message.contains("no shape")),
            "got: {said:?}"
        );

        // A list whose items the schema says nothing about, and a table whose keys
        // are the author's own — the two ways a walk carries on without a shape.
        assert!(held(Toml::Array(vec![Toml::Integer(1)]), r#"{"type":"array"}"#).is_empty());
        let free = r#"{"type":"object","additionalProperties":{"type":"string"}}"#;
        let mut table = toml::map::Map::new();
        table.insert("anything".to_owned(), Toml::Integer(1));
        assert!(!held(Toml::Table(table.clone()), free).is_empty());

        // And the two shapes a table can be that constrain nothing: one naming no
        // fields and saying nothing about the rest, and an open record, which takes
        // the fields it names and tolerates the others. A schema this build generates
        // is neither today, and a walk that refused them would be refusing the format
        // its own types could grow into.
        assert!(held(Toml::Table(table.clone()), r#"{"type":"object"}"#).is_empty());
        let open = r#"{"type":"object","properties":{"named":{"type":"string"}}}"#;
        let mut record = toml::map::Map::new();
        record.insert("named".to_owned(), Toml::String("x".to_owned()));
        record.insert("beside".to_owned(), Toml::Integer(1));
        assert!(held(Toml::Table(record), open).is_empty());
    }

    /// Every kind of value has a name, and every kind a schema asks for has one too.
    ///
    /// Both lists are read out of a match, and a match arm nothing reaches is a word
    /// an author would be shown that nobody has ever seen.
    #[test]
    fn every_kind_is_named_in_the_words_its_side_uses() {
        let named_as: Vec<&str> = [
            Toml::String(String::new()),
            Toml::Integer(0),
            Toml::Float(0.0),
            Toml::Boolean(false),
            Toml::Array(Vec::new()),
            Toml::Table(toml::map::Map::new()),
        ]
        .iter()
        .map(named)
        .collect();
        assert_eq!(
            named_as,
            vec![
                "a string",
                "a whole number",
                "a number",
                "a true or a false",
                "a list",
                "a table"
            ]
        );

        for (asked, shown) in [
            ("string", "a string"),
            ("integer", "a whole number"),
            ("number", "a number"),
            ("boolean", "a true or a false"),
            ("array", "a list"),
            ("object", "a table"),
            ("oddity", "oddity"),
        ] {
            assert_eq!(wanted(&[asked, "null"]), shown, "asked for {asked}");
        }
    }

    /// A value is said back as an author wrote it, and a shape has no spelling.
    #[test]
    fn a_value_is_quoted_as_a_word_and_a_shape_is_named_as_one() {
        assert_eq!(spoken(&Toml::String("wan".to_owned())), "`wan`");
        assert_eq!(spoken(&Toml::Integer(7)), "7");
        assert_eq!(spoken(&Toml::Boolean(true)), "true");
        assert_eq!(spoken(&Toml::Array(Vec::new())), "a list");
        assert_eq!(rendered(&Json::Bool(true)), "true");
        assert!(!is(&Toml::Float(1.0), &Json::Null));
    }

    /// A fault with nowhere to place it reads as the message alone.
    #[test]
    fn a_violation_about_the_whole_file_is_not_prefixed_with_an_empty_place() {
        let one = Violation {
            location: String::new(),
            message: "the file is not a manifest".to_owned(),
        };
        assert_eq!(one.to_string(), "the file is not a manifest");
    }

    /// Several names in one line read as a list, whatever kind of thing they are.
    #[test]
    fn names_are_listed_in_one_stable_order_without_repeats() {
        let three = ["b", "a", "b"].into_iter().map(str::to_owned);
        assert_eq!(listed(three), "a, b");
        assert_eq!(listed(["only".to_owned()].into_iter()), "only");
    }

    /// A date is a kind of value TOML has and this format declares nowhere.
    #[test]
    fn a_kind_the_format_never_declares_is_still_named_rather_than_guessed_at() {
        let stamp = "1979-05-27T07:32:00Z"
            .parse::<toml::value::Datetime>()
            .ok()
            .map(Toml::Datetime);
        assert_eq!(stamp.as_ref().map(named), Some("a date"));
    }

    /// Every field the published schema declares anywhere in it.
    fn every_field(schema: &Json, into: &mut BTreeSet<String>) {
        match schema {
            Json::Object(table) => {
                if let Some(Json::Object(properties)) = table.get("properties") {
                    into.extend(properties.keys().cloned());
                }
                for (key, value) in table {
                    if key != "description" && key != "title" {
                        every_field(value, into);
                    }
                }
            }
            Json::Array(entries) => {
                for entry in entries {
                    every_field(entry, into);
                }
            }
            _ => {}
        }
    }

    /// A plugin is data, and the format is what makes that true rather than restraint.
    ///
    /// There is no field in which a plugin could ask to run something, reach the machine,
    /// or hand over content nobody can read — so a manifest trying is malformed rather
    /// than a permission being withheld. Asked of the generated schema, because that is
    /// the document an author writes against and an editor enforces.
    #[test]
    fn the_format_has_no_field_for_code_for_reach_or_for_content_nobody_can_read() {
        const ABSENT: &[&str] = &[
            "command",
            "entrypoint",
            "exec",
            "script",
            "shell",
            "environment",
            "env",
            "grants",
            "cap_add",
            "devices",
            "privileged",
            "network_mode",
            "user",
            "volumes",
            "mounts",
            "depends_on",
            "extends",
            "profile",
            "host_managed",
            "payload",
            "blob",
            "base64",
            "binary",
            "plugin_path",
            "library",
        ];
        let published = published();
        let mut named = BTreeSet::new();
        every_field(published.as_value(), &mut named);
        let counted = named.len();
        assert!(
            counted > 40,
            "the schema was read and it declares {counted} fields"
        );
        for absent in ABSENT {
            assert!(
                !named.contains(*absent),
                "the format declares `{absent}`, which is a way to reach past what it says"
            );
        }
    }

    /// What a plugin is, as the set of blocks it may write.
    ///
    /// Held to the whole set rather than to a floor, so that a block added without the
    /// feature saying so fails here — and so does one quietly removed.
    #[test]
    fn a_manifest_is_the_ten_kinds_of_declaration_and_no_other() {
        let published = published();
        let blocks: BTreeSet<&str> = published
            .as_value()
            .get("properties")
            .and_then(Json::as_object)
            .map(|declared| declared.keys().map(String::as_str).collect())
            .unwrap_or_default();
        assert_eq!(
            blocks,
            BTreeSet::from([
                "schema_version",
                "plugin",
                "service",
                "claim",
                "wiring",
                "proof",
                "contribution",
                "recipe",
                "secret",
                "override",
                "requires",
            ])
        );
    }

    /// The walk reads a generated schema, and a schema is a thing that can go stale.
    ///
    /// A manifest carrying every block the contract declares has to come back clean, or
    /// the sweep above is refusing something correct — which is the failure nothing else
    /// here could show, because every other test asks it about a fault.
    #[test]
    fn a_manifest_declaring_every_block_is_refused_nothing() {
        assert_eq!(
            nonconforming(crate::schema::tests::WHOLE),
            Vec::new(),
            "the whole manifest conforms"
        );
    }
}
