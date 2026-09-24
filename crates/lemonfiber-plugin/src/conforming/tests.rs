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
        "cap_drop",
        "devices",
        "device_cgroup_rules",
        "privileged",
        "network_mode",
        "networks",
        "user",
        "userns_mode",
        "group_add",
        "volumes",
        "volumes_from",
        "mounts",
        "tmpfs",
        "security_opt",
        "sysctls",
        "ipc",
        "pid",
        "env_file",
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

/// What a plugin's service may declare, and the whole of what it may declare.
///
/// The complement of the rule above, and it is here because a deny-list cannot
/// close a format on its own: a field added under a name nobody thought to ban
/// passes that one and is read by the reader all the same. The set of fields a
/// service declares *is* the set of things a plugin may ask for, so it is held as
/// a set — a field added to this table fails here naming itself, whatever it is
/// called, and whoever added it has to say why it belongs rather than why it was
/// not banned.
///
/// The service table alone, because it is the only block from which a container
/// is written. What the other blocks may not carry is the list above, which is
/// asked of every table there is.
#[test]
fn a_plugins_service_declares_exactly_the_fields_the_contract_permits() {
    const PERMITTED: &[&str] = &[
        "bind",
        "config_path",
        "criticality",
        "digest",
        "health",
        "id",
        "image",
        "media_types",
        "name",
        "port",
        "provides",
        "tag",
        "takes_data",
    ];
    let published = published();
    let declared: BTreeSet<String> = published
        .as_value()
        .pointer("/$defs/PluginService/properties")
        .and_then(Json::as_object)
        .map(|held| held.keys().cloned().collect())
        .unwrap_or_default();
    let permitted: BTreeSet<String> = PERMITTED.iter().map(|&one| one.to_owned()).collect();
    assert_eq!(declared, permitted);
}

/// A plugin cannot say what reaches what.
///
/// The stack manifest declares a link between two of its services, and a link
/// that names a service rather than asking for a capability is the exception an
/// operator makes deliberately. A plugin has no field for either: it declares
/// what its own service can do and is reached by whatever already asks, and the
/// route and the dashboard entry are written for it from its tier.
///
/// Asked of the generated schema rather than of the types, because the document
/// an author writes against is the one that has to have no such field — and
/// because a field added to a nested block would satisfy a check that only read
/// the top level.
///
/// A recipe's `to` is not one of these and is deliberately left off the list: it
/// names where a captured value goes inside the plugin's own recipe, and what a
/// call may address is bounded where recipes are read.
#[test]
fn the_format_has_no_field_by_which_a_plugin_could_wire_to_a_named_service() {
    const ABSENT: &[&str] = &[
        "asks",
        "by",
        "by_name",
        "depends_on",
        "each",
        "filled_by",
        "fills",
        "reaches",
        "wire",
        "wires",
    ];
    let published = published();
    let mut named = BTreeSet::new();
    every_field(published.as_value(), &mut named);
    for absent in ABSENT {
        assert!(
            !named.contains(*absent),
            "the format declares `{absent}`, which is a way to say what reaches what"
        );
    }
    // The one spelling it does declare, so the sweep above is read against a
    // format that has a `wiring` block rather than against one that has none.
    assert!(named.contains("wiring"));
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
