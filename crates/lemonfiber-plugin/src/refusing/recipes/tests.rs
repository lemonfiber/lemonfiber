use super::looks_like_an_address;
use crate::addressing::named as substitutions;
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
         capture = [{ name = \"token\", from = \"token\", origin = \"stack-service\" }]\n\n\
         [[recipe.pair]]\nvalue = \"token\"\nto    = \"komga\"",
    );
    let twice_said = said(&twice);
    assert!(
        names(&twice_said, &["step sign-in", "declared twice"]),
        "got: {twice_said:?}"
    );
    assert!(
        names(&twice_said, &["capture", "token", "twice"]),
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

/// A header's name is a fixed identifier of the protocol, written out like the path, so
/// a substitution there is refused for where it stands and is not also read as a flow.
#[test]
fn a_value_substituted_into_a_header_s_name_is_refused_where_it_stands() {
    for header in ["X-{{nothing}}", "X-{{token}}", "{{token}}"] {
        let said = without(
            r#"headers = { Authorization = "Bearer {{token}}" }"#,
            &format!(r#"headers = {{ "{header}" = "1" }}"#),
        );
        assert!(
            names(&said, &["call.headers", header, "fixed identifier"]),
            "{header}: {said:?}"
        );
        assert!(
            !names(&said, &["call.headers", "no earlier step"]),
            "{header}: {said:?}"
        );
        let manifest = Manifest::from_toml(&WHOLE.replace(
            r#"headers = { Authorization = "Bearer {{token}}" }"#,
            &format!(r#"headers = {{ "{header}" = "1" }}"#),
        ));
        assert!(manifest.is_ok_and(|one| crate::names_a_header_by_substitution(&one)));
    }
}

/// A substitution in a header's value is what headers are for, and is not a name.
#[test]
fn a_value_substituted_into_a_header_s_value_names_no_header() {
    let manifest = Manifest::from_toml(WHOLE);
    assert!(manifest.is_ok_and(|one| !crate::names_a_header_by_substitution(&one)));
    let said = without(
        r#"headers = { Authorization = "Bearer {{token}}" }"#,
        r#"headers = { Authorization = "Bearer {{token}}", X-Plain = "{{token}}" }"#,
    );
    assert!(!names(&said, &["fixed identifier"]), "{said:?}");
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

/// A title or a reason carrying an escape sequence or a carriage return is refused: it
/// is printed to a terminal beside what an operator approves, and either would redraw
/// that line.
#[test]
fn a_recipe_whose_words_instruct_a_terminal_is_refused() {
    for (before, after) in [
        (
            "title = \"Point it at the comics the stack already files\"",
            "title = \"Point it \\u001b[2Kat the comics\"",
        ),
        (
            "why   = \"The stack already files comics, and a fresh Komga knows nothing about them.\"",
            "why   = \"The stack already files comics\\rApproved.\"",
        ),
    ] {
        let said = without(before, after);
        assert!(names(&said, &["recipe adopt-existing-library"]), "got: {said:?}");
    }
}

/// A value, a recipe id, a step id or a capture name that is not one word is refused,
/// so what an approval names and what it matches are the same bytes.
#[test]
fn a_name_an_approval_is_written_with_must_be_one_word() {
    for (before, after, location) in [
        (
            "value = \"token\"",
            "value = \"tok\\u202eneko\"",
            "pair #1.value",
        ),
        (
            "value = \"token\"",
            "value = \"tok\\u001b[2Ken\"",
            "pair #1.value",
        ),
        ("id      = \"sign-in\"", "id      = \"sign in\"", "id"),
        (
            "name = \"token\", from",
            "name = \"to\\u200bken\", from",
            "capture.name",
        ),
    ] {
        let said = without(before, after);
        assert!(names(&said, &[location, "one word"]), "got: {said:?}");
    }
}

/// A destination is a service id or a lowercase host name and nothing else: free text,
/// a name with a capital in it, and an address are each refused.
#[test]
fn a_destination_must_be_a_name_as_it_is_written() {
    for written in ["Komga", "kom ga", "komga\\u202e", "-komga", "[::1]"] {
        let said = without("to    = \"komga\"", &format!("to    = \"{written}\""));
        assert!(
            names(&said, &["pair #1.to", "not a service id"]),
            "{written}: {said:?}"
        );
    }
    let said = without(
        "to = \"komga\", path = \"/api/v1/login\"",
        "to = \"Komga.Example\", path = \"/api/v1/login\"",
    );
    assert!(
        names(&said, &["call.to", "not a service id"]),
        "got: {said:?}"
    );
}

/// A host name longer than DNS allows, or with a label longer than one may be, is not a
/// name.
#[test]
fn a_destination_longer_than_a_name_can_be_is_not_one() {
    assert!(!super::is_name(&format!("{}.example", "a".repeat(64))));
    assert!(!super::is_name(&"a.".repeat(127)));
    assert!(super::is_name("metadata.example.org"));
}

/// A path, a header and what a capture reads carry nothing a diff cannot show.
#[test]
fn every_field_a_call_and_a_capture_declare_is_held_to_being_readable() {
    for (before, after) in [
        (
            "path = \"/api/v1/login\"",
            "path = \"/api/v1/\\u001b[2Klogin\"",
        ),
        (
            "Authorization = \"Bearer {{token}}\"",
            "Authorization = \"Bearer\\r{{token}}\"",
        ),
        ("from = \"token\"", "from = \"to\\u0007ken\""),
        ("ask    = \"What", "ask    = \"\\u001b[2KWhat"),
        ("every = \"5s\"", "every = \"5\\u0007s\""),
        ("equals = \"Comics\"", "equals = \"Com\\u202eics\""),
    ] {
        let said = without(before, after);
        assert!(names(&said, &["diff cannot show"]), "{after}: {said:?}");
    }
}

/// A query value may carry a value a pair permits, and nothing else in a path may.
#[test]
fn only_a_query_value_in_a_path_carries_a_value() {
    let carried = without(
        "path = \"/api/v1/libraries\"",
        "path = \"/api/v1/libraries?token={{token}}&kind=comics\"",
    );
    assert!(!names(&carried, &["call.path"]), "got: {carried:?}");

    let unpaired = without(
        "path = \"/api/v1/libraries\"",
        "path = \"/api/v1/libraries?name={{library-name}}\"",
    );
    assert!(
        names(
            &unpaired,
            &["call.path", "library-name", "no [[recipe.pair]]"]
        ),
        "got: {unpaired:?}"
    );

    for path in [
        "/api/{{nothing}}/libraries",
        "/api/v1/libraries?{{nothing}}=x",
        "/api/v1/libraries?{{nothing}}",
    ] {
        let said = without(
            "path = \"/api/v1/libraries\"",
            &format!("path = \"{path}\""),
        );
        assert!(
            names(&said, &["call.path", "only a query value"]),
            "{path}: {said:?}"
        );
        assert!(
            !names(&said, &["call.path", "no earlier step"]),
            "{path}: a refused place is not also read as a flow: {said:?}"
        );
    }
}

/// Where a call goes is written out, and never substituted.
#[test]
fn a_destination_carrying_a_substitution_is_refused() {
    let said = without(
        r#"to = "komga", path = "/api/v1/libraries""#,
        r#"to = "{{token}}", path = "/api/v1/libraries""#,
    );
    assert!(names(&said, &["call.to", "written out"]), "got: {said:?}");
}

/// A pair is a value the recipe has, to somewhere that is a destination.
#[test]
fn a_pair_naming_no_value_or_no_destination_is_refused() {
    let valueless = without(
        "value = \"token\"\nto    = \"komga\"",
        "value = \"nothing\"\nto    = \"komga\"",
    );
    assert!(
        names(&valueless, &["pair #1.value", "nothing", "permits nothing"]),
        "got: {valueless:?}"
    );
    let nowhere = without(
        "value = \"token\"\nto    = \"komga\"",
        "value = \"token\"\nto    = \"elsewhere\"",
    );
    assert!(
        names(&nowhere, &["pair #1.to", "elsewhere", "neither a service"]),
        "got: {nowhere:?}"
    );
}

/// An input and a capture of one name are two values one name means.
#[test]
fn an_input_and_a_capture_of_one_name_are_refused() {
    let said = without("name   = \"library-name\"", "name   = \"token\"");
    assert!(
        names(&said, &["capture", "token", "twice"]),
        "got: {said:?}"
    );
    let twice = without(
        "[[recipe.step]]\nid      = \"sign-in\"",
        "[[recipe.input]]\nname   = \"library-name\"\norigin = \"operator\"\nask    = \"Again\"\n\n\
         [[recipe.step]]\nid      = \"sign-in\"",
    );
    assert!(
        names(&twice, &["input library-name", "twice"]),
        "got: {twice:?}"
    );
}

/// Whether a destination is outside the stack is answered as validation classifies it.
#[test]
fn a_destination_is_outside_where_it_is_a_host_and_inside_where_it_is_a_service() {
    let Some(manifest) = Manifest::from_toml(WHOLE).ok() else {
        unreachable!("the whole fixture parses");
    };
    assert!(super::outside(&manifest, "metadata.example.org"));
    assert!(!super::outside(&manifest, "komga"));
    assert!(!super::outside(&manifest, "sonarr"));
    assert!(!super::outside(&manifest, "elsewhere"));
}
