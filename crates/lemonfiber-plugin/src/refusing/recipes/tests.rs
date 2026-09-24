use super::{looks_like_an_address, substitutions};
use crate::refusing::refusals;
use crate::schema::tests::WHOLE;
use crate::schema::Manifest;

/// The identities lemonfiber's own registers hold, which the whole fixture avoids.
const OCCUPIED: &[&str] = &["storage.hardlinks"];

fn said(text: &str) -> Vec<String> {
    Manifest::from_toml(text).map_or_else(
        |refused| vec![refused.to_string()],
        |manifest| {
            refusals(&manifest, OCCUPIED)
                .iter()
                .map(ToString::to_string)
                .collect()
        },
    )
}

fn names(said: &[String], words: &[&str]) -> bool {
    said.iter()
        .any(|one| words.iter().all(|word| one.contains(word)))
}

fn without(before: &str, after: &str) -> Vec<String> {
    assert!(WHOLE.contains(before), "the fixture still says {before:?}");
    said(&WHOLE.replace(before, after))
}

/// The declared block is read rather than skipped, and the capability is named.
#[test]
fn a_recipe_declared_without_the_capability_that_runs_it_is_refused_by_naming_it() {
    let said = without(r#""recipe.run""#, r#""doctor.contribute""#);
    assert!(
        names(&said, &["requires.capabilities", "recipe.run"]),
        "got: {said:?}"
    );
    assert!(
        !said.iter().any(|one| one.contains("version")),
        "and never by naming a version: {said:?}"
    );
}

/// A manifest with no recipe is not asked for the capability that runs one.
#[test]
fn a_manifest_declaring_no_recipe_is_not_asked_for_it() {
    let text = WHOLE
        .split("[[recipe]]")
        .next()
        .unwrap_or_default()
        .replace(r#", "recipe.run""#, "");
    let said = said(&format!(
        "{text}\n[requires]\ncapabilities = [\"doctor.contribute\"]\n"
    ));
    assert!(
        !names(&said, &["recipe.run"]),
        "nothing asks for it: {said:?}"
    );
}

#[test]
fn a_value_carried_to_a_destination_no_pair_declares_is_refused() {
    let said = without(
        "[[recipe.pair]]\nvalue = \"token\"\nto    = \"komga\"",
        "[[recipe.pair]]\nvalue = \"token\"\nto    = \"elsewhere\"",
    );
    assert!(
        names(
            &said,
            &["recipe adopt-existing-library.step create", "token"]
        ),
        "got: {said:?}"
    );
}

#[test]
fn a_substitution_no_earlier_step_captures_is_refused() {
    let said = without("{{token}}", "{{nothing}}");
    assert!(names(&said, &["nothing"]), "got: {said:?}");
}

#[test]
fn a_call_to_an_address_rather_than_a_name_is_refused() {
    let said = without(
        r#"to = "komga", path = "/api/v1/libraries""#,
        r#"to = "10.0.0.5", path = "/api/v1/libraries""#,
    );
    assert!(names(&said, &["call.to", "10.0.0.5"]), "got: {said:?}");
}

#[test]
fn a_method_outside_the_verbs_a_call_may_use_is_refused() {
    let said = without(
        r#"call    = { method = "POST", to = "komga", path = "/api/v1/libraries""#,
        r#"call    = { method = "TRACE", to = "komga", path = "/api/v1/libraries""#,
    );
    assert!(names(&said, &["call.method", "TRACE"]), "got: {said:?}");
}

/// Two of anything a verdict is reported against is two things one name means.
#[test]
fn a_recipe_a_step_or_a_capture_declared_twice_is_refused() {
    let twice = WHOLE.replace(
        "[[recipe.pair]]\nvalue = \"token\"\nto    = \"komga\"",
        "[[recipe.step]]\nid      = \"sign-in\"\n\
         call    = { method = \"GET\", to = \"komga\", path = \"/again\" }\n\
         capture = [{ name = \"token\", from = \"json.token\", origin = \"stack-service\" }]\n\n\
         [[recipe.pair]]\nvalue = \"token\"\nto    = \"komga\"",
    );
    let twice_said = said(&twice);
    assert!(
        names(&twice_said, &["step sign-in", "declared twice"]),
        "got: {twice_said:?}"
    );
    assert!(
        names(&twice_said, &["capture", "captured twice"]),
        "got: {twice_said:?}"
    );

    let doubled = format!(
        "{WHOLE}\n[[recipe]]\nid    = \"adopt-existing-library\"\n\
         title = \"t\"\nwhy   = \"w\"\n"
    );
    let doubled_said = said(&doubled);
    assert!(
        names(
            &doubled_said,
            &["recipe adopt-existing-library", "declared twice"]
        ),
        "got: {doubled_said:?}"
    );
}

/// A body carries a value exactly as a header does, and is read the same way.
#[test]
fn a_value_substituted_into_a_body_is_read_as_a_flow_too() {
    let said = without(
        r#"body = "{\"name\": \"Comics\"}" }"#,
        r#"body = "{\"name\": \"{{nothing}}\"}" }"#,
    );
    assert!(names(&said, &["call.body", "nothing"]), "got: {said:?}");
}

/// A file the schema refuses never reaches these rules.
#[test]
fn a_manifest_that_is_not_one_is_answered_by_the_reader_rather_than_here() {
    let said = said("schema_version = 1\n[plugin]\nid = \"komga\"\n");
    assert!(names(&said, &["does not conform"]), "got: {said:?}");
}

#[test]
fn an_address_is_told_from_a_name_by_its_last_label() {
    assert!(looks_like_an_address("10.0.0.5"));
    assert!(looks_like_an_address("komga:25600"));
    assert!(!looks_like_an_address("komga"));
    assert!(!looks_like_an_address("api.example.com"));
}

#[test]
fn a_substitution_is_read_out_of_the_text_it_sits_in() {
    let found: Vec<&str> = substitutions("Bearer {{token}} for {{ library }}").collect();
    assert_eq!(found, vec!["token", "library"]);
    assert_eq!(substitutions("nothing here").count(), 0);
}
