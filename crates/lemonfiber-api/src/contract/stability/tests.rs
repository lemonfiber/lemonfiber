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
        strings: BTreeSet::new(),
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
        strings: BTreeSet::new(),
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

/// A surface whose one field is spelled `spelled`, beside a described type named
/// `Set` holding these constants, and read as admitting nothing but string
/// constants where `strings_only` says its schema did.
fn beside_a_set(spelled: &str, constants: &[&str], strings_only: bool) -> Surface {
    let mut surface = one_field("reversal", spelled, true);
    surface.types.insert(
        "Set".to_owned(),
        Shape {
            fields: BTreeMap::new(),
            variants: constants.iter().map(|one| (*one).to_owned()).collect(),
        },
    );
    if strings_only {
        surface.strings.insert("Set".to_owned());
    }
    surface
}

/// A bare string that comes to name a closed set of strings is a narrowing: every
/// value it can now carry is a string it could carry before, so nothing a consumer
/// parses fails.
#[test]
fn a_string_that_becomes_a_closed_set_of_strings_is_not_a_break() {
    let before = one_field("reversal", "string", true);
    let after = beside_a_set("ref:Set", &["\"whole\"", "\"partial\"", "\"none\""], true);
    assert_eq!(Surface::broken(&before, &after), Vec::new());
}

/// A reference is otherwise still compared by name. A string that comes to point at
/// a definition whose schema admits anything but string constants — or at a name
/// that is not such a definition at all — has changed what arrives.
#[test]
fn a_string_that_becomes_anything_else_by_reference_is_still_a_break() {
    let before = one_field("reversal", "string", true);
    for after in [
        beside_a_set("ref:Set", &["\"whole\""], false),
        beside_a_set("ref:Elsewhere", &["\"whole\""], true),
    ] {
        assert_eq!(Surface::broken(&before, &after).len(), 1, "{after:?}");
    }
}

/// Only a bare string narrows. A field that was a number does not become one by
/// pointing at a set of strings.
#[test]
fn only_a_bare_string_is_narrowed() {
    let before = one_field("reversal", "integer", true);
    let after = beside_a_set("ref:Set", &["\"whole\""], true);
    assert_eq!(Surface::broken(&before, &after).len(), 1);
}

/// The set it narrowed to is a described type like any other, so a value taken out
/// of it later is refused.
#[test]
fn a_value_taken_out_of_the_set_it_narrowed_to_is_still_a_break() {
    let before = beside_a_set("ref:Set", &["\"whole\"", "\"none\""], true);
    let after = beside_a_set("ref:Set", &["\"whole\""], true);
    let broken = Surface::broken(&before, &after);
    assert!(
        broken.iter().any(|one| one.what == "Set = \"none\""),
        "{broken:?}"
    );
}

/// Which definitions admit nothing but string constants is read off the schema, in
/// both spellings a generator writes a set of words in — and a set with anything
/// else beside it is not one, which is the case a shape alone could not see.
#[test]
fn a_set_of_words_is_told_from_a_set_with_something_else_beside_it() {
    let words = [
        json!({"oneOf": [{"type": "string", "const": "whole"}, {"const": "none"}]}),
        json!({"type": "string", "enum": ["whole", "none"]}),
        json!({"anyOf": [{"type": "string", "const": "whole", "description": "all of it"}]}),
    ];
    for schema in &words {
        assert!(super::only_string_constants(schema), "{schema}");
    }
    let not_only_words = [
        json!({"oneOf": [{"type": "string", "const": "whole"}, {"type": "integer"}]}),
        json!({"oneOf": [{"type": "string", "const": "whole"}, {"const": 1}]}),
        json!({"oneOf": [{"type": "object", "const": "x", "properties": {}}]}),
        json!({"oneOf": [{"type": "string", "const": "x", "properties": {"a": {}}}]}),
        json!({"oneOf": []}),
        json!({"type": "string", "enum": ["whole", 1]}),
        json!({"type": "integer", "enum": ["whole"]}),
        json!({"type": "string", "enum": []}),
        json!({"type": "string"}),
        json!({"type": "object", "properties": {"whole": {"type": "string"}}}),
    ];
    for schema in &not_only_words {
        assert!(!super::only_string_constants(schema), "{schema}");
    }
}

/// And the reading of a real contract records the one this change introduced, so
/// the rule above is exercised against what the generator actually wrote.
#[test]
fn the_contract_s_own_set_of_reversals_is_read_as_a_set_of_words() {
    let fresh = Surface::of(&Contract::describe());
    assert!(
        fresh.strings.contains("ChangeReversal"),
        "{:?}",
        fresh.strings
    );
    assert!(
        !fresh.strings.contains("ChangeReport"),
        "and a report with fields is not"
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

    // Compared as what is committed. The reading of which definitions are sets of
    // words is held in memory beside a fresh surface and is never written, so a
    // stored surface has none and comparing the values would compare that too.
    assert_eq!(
        stored.to_json(),
        fresh.to_json(),
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
