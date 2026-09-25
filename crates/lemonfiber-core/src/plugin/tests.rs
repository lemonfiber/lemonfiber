use std::collections::BTreeSet;

use super::{
    capabilities_of, points, schema, vocabulary, Ungenerated, POINTS_PATH, SCHEMA_PATH, STACK,
    VOCABULARY_PATH,
};
use crate::doctor::Category;

/// What is committed at a path, read from the workspace root.
fn committed(path: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read_to_string(root.join(path)).unwrap_or_default()
}

/// The committed schema and the types must agree.
///
/// A field added to a plugin manifest without regenerating fails here rather than
/// reaching an author's editor as a schema that describes a reader this build is
/// not.
#[test]
fn the_committed_schema_still_matches_the_types() {
    let fresh = schema().unwrap_or_default();
    assert_eq!(
        committed(SCHEMA_PATH),
        fresh,
        "the plugin manifest schema is out of date — regenerate it with `just plugin-schema`"
    );
}

/// The committed vocabulary and the stack this build pins must agree.
///
/// Moving the stack pin can move this file, which is the intended behaviour: a
/// bundled service that stops declaring a capability changes what lemonfiber
/// publishes, and a change nobody meant shows up as a diff here rather than as a
/// plugin that stopped wiring six months later.
#[test]
fn the_committed_vocabulary_still_matches_the_types_and_the_pinned_stack() {
    let fresh = vocabulary().unwrap_or_default();
    assert_eq!(
        committed(VOCABULARY_PATH),
        fresh,
        "the capability vocabulary is out of date — regenerate it with `just capabilities`"
    );
}

/// The committed points and the register they name must agree.
#[test]
fn the_committed_extension_points_still_match_the_register() {
    let fresh = points().unwrap_or_default();
    assert_eq!(
        committed(POINTS_PATH),
        fresh,
        "the extension points are out of date — regenerate them with `just extension-points`"
    );
}

/// A stack that is not a stack description fails generation rather than writing an
/// artefact with nothing in `declared_by`.
#[test]
fn a_stack_this_build_cannot_read_fails_generation() {
    assert!(matches!(
        capabilities_of("= not toml"),
        Err(Ungenerated::Stack(_))
    ));
}

/// The pinned stack with one capability taken out of every service declaring it.
///
/// The comma goes with the name. A service declaring two leaves `[, "other"]`
/// behind otherwise, which is a stack that does not parse — and a test that then
/// proves the reader refuses bad TOML rather than what it was written for.
///
/// Only the declarations, for the same reason. The stack also *asks* for these
/// names, and an edit that reached those lines would leave `asks =` with nothing
/// after it — the same stack that will not parse, arrived at a different way.
fn without(name: &str) -> String {
    STACK
        .lines()
        .map(|line| {
            if !line.trim_start().starts_with("provides = ") {
                return line.to_owned();
            }
            line.replace(&format!("\"{name}\", "), "")
                .replace(&format!(", \"{name}\""), "")
                .replace(&format!("\"{name}\""), "")
        })
        .collect::<Vec<String>>()
        .join("\n")
}

/// A capability no bundled service declares fails generation, naming it.
///
/// Every one of them in turn rather than the first, and not only for thoroughness:
/// "the first" is an option, and an arm for a vocabulary carrying nothing is a line
/// no run can ever enter.
#[test]
fn a_capability_nothing_declares_fails_generation_by_name() {
    let mut asked = 0;
    for held in lemonfiber_plugin::vocabulary::carried() {
        let said = capabilities_of(&without(held.name))
            .err()
            .map(|refused| refused.to_string())
            .unwrap_or_default();
        assert!(said.contains(held.name), "got: {said}");
        assert!(
            said.contains("declared by no bundled service"),
            "got: {said}"
        );
        asked += 1;
    }
    assert!(asked > 1, "the vocabulary carries more than one capability");
}

/// A bundled service declaring a name the vocabulary lacks fails generation,
/// naming both.
#[test]
fn a_service_declaring_a_name_the_vocabulary_lacks_fails_generation_by_name() {
    let mut asked = 0;
    for held in lemonfiber_plugin::vocabulary::carried() {
        let odd = STACK.replacen(
            &format!("\"{}\"", held.name),
            &format!("\"{}\", \"nothing.here\"", held.name),
            1,
        );
        let said = capabilities_of(&odd)
            .err()
            .map(|refused| refused.to_string())
            .unwrap_or_default();
        assert!(said.contains("nothing.here"), "got: {said}");
        asked += 1;
    }
    assert!(asked > 1, "the vocabulary carries more than one capability");
}

/// The failure that cannot happen still says what it would mean.
///
/// A rendering nothing runs is a rendering nothing holds to being readable, and
/// this one reaches whoever ran the generator.
#[test]
fn an_artefact_that_would_not_serialise_says_so() {
    assert_eq!(
        Ungenerated::Unrenderable.to_string(),
        "the artefact could not be written as JSON"
    );
}

/// The families a contributed check may declare are the doctor's own nine.
///
/// Published beside the row rather than read from the register, because the row's
/// closed sets are part of what the point declares — so this is the comparison
/// that keeps the second copy from being a second answer.
#[test]
fn the_families_a_contribution_may_declare_are_the_ones_the_doctor_recognises() {
    let published: BTreeSet<&str> = lemonfiber_plugin::extension::categories()
        .iter()
        .copied()
        .collect();
    let recognised: BTreeSet<&str> = Category::every()
        .into_iter()
        .map(Category::as_str)
        .collect();
    assert!(!recognised.is_empty(), "there are families to compare");
    assert_eq!(published, recognised);
}
