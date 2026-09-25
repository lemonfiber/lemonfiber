// The fixtures the whole of `refusing` is driven against: a manifest this build
// would act on, the one edit each case makes to it, and the two ways of reading
// what came back. Shared rather than copied, so a rule proved against a fixture
// nobody else uses is a rule proved against nothing in particular.
use crate::refusing::tests::{names, said, without, INSTALLABLE};

/// The representations a request may ask for, each one a service actually sends.
#[test]
fn a_request_may_ask_for_one_media_type() {
    for accept in [
        "application/json",
        "application/vnd.api+json",
        "application/json; charset=utf-8",
        "text/xml",
        "application/opds+json;profile=opds-catalog",
    ] {
        let said = without(
            r#"request = { method = "GET", path = "/api/v1/series" }
expect  = { status = 401 }"#,
            &format!(
                "request = {{ method = \"GET\", path = \"/api/v1/series\", \
                     accept = \"{accept}\" }}\nexpect  = {{ status = 401 }}"
            ),
        );
        assert!(said.is_empty(), "{accept}: {said:?}");
    }
}

/// And the ways one is not a media type, each refused naming the value.
#[test]
fn a_request_asking_for_anything_but_one_media_type_is_refused_by_name() {
    for (accept, words) in [
        ("application/json, text/xml", "more than one representation"),
        ("*/*", "wildcard"),
        ("application/*", "wildcard"),
        ("json", "is not a media type"),
        ("application/", "leaves one half"),
        ("/json", "leaves one half"),
        ("application/json; charset", "parameter that is not one"),
        ("application/json@1", "which a media type may not"),
        ("application/js on", "which a media type may not"),
        // A control character would not survive the wire, and is refused here by
        // the same rule rather than by the sweep that reads every declared string:
        // this one names the field the author wrote it in.
        ("application/json\\u007F", "which a media type may not"),
    ] {
        let said = without(
            r#"request = { method = "GET", path = "/api/v1/series" }
expect  = { status = 401 }"#,
            &format!(
                "request = {{ method = \"GET\", path = \"/api/v1/series\", \
                     accept = \"{accept}\" }}\nexpect  = {{ status = 401 }}"
            ),
        );
        assert!(
            names(&said, &["accept", words]),
            "{accept} was not refused for {words}: {said:?}"
        );
    }
}

/// The places an expectation may look, including the one Plex needs.
#[test]
fn an_expectation_may_look_wherever_a_pointer_reaches() {
    for key in [
        "content",
        "/content",
        "/MediaContainer/size",
        "/MediaContainer/Setting/[id=PublishServerOnPlexOnlineKey]/value",
        "/a~1b",
    ] {
        let said = without(
            r#"json_has_keys = ["content"], json_types = { content = "list" }"#,
            &format!(r#"json_has_keys = ["{key}"], json_types = {{ content = "list" }}"#),
        );
        assert!(said.is_empty(), "{key}: {said:?}");
    }
}

/// A key naming no place is refused when the manifest is read, never evaluated.
///
/// Each of the four key-wise constraints, because a rule that looked at one of them
/// would leave the other three carrying keys nothing can reach.
#[test]
fn an_expectation_key_naming_no_place_is_refused_by_name() {
    for (before, after, field) in [
        (
            r#"json_has_keys = ["content"]"#,
            r#"json_has_keys = ["/Setting[id=x]"]"#,
            "json_has_keys",
        ),
        (
            r#"json_types = { content = "list" }"#,
            r#"json_types = { "/a~2b" = "list" }"#,
            "json_types",
        ),
        (
            r#"expect  = { status = 200, json = { status = "UP" } }"#,
            r#"expect  = { status = 200, json = { "/[=x]" = "UP" } }"#,
            "json",
        ),
        (
            r#"expect  = { status = 200, json = { status = "UP" } }"#,
            r#"expect  = { status = 200, json_at_least = { "/a~" = 1 } }"#,
            "json_at_least",
        ),
    ] {
        let said = without(before, after);
        assert!(
            names(&said, &[field, "names no place in an answer"]),
            "{after}: {said:?}"
        );
    }
}

/// A recipe step's expectation is held to it too, where a step's call is not.
#[test]
fn a_recipe_steps_expectation_is_held_to_the_same_places() {
    let text = format!(
        "{INSTALLABLE}\n[[recipe]]\nid = \"seed\"\ntitle = \"Seed\"\nwhy = \"Because\"\n\
             [[recipe.step]]\nid = \"one\"\n\
             call = {{ method = \"GET\", to = \"komga\", path = \"/x\" }}\n\
             expect = {{ status = 200, json_has_keys = [\"/a~\"] }}\n"
    );
    let said = said(&text);
    assert!(
        names(
            &said,
            &["recipe seed.step one", "names no place in an answer"]
        ),
        "got: {said:?}"
    );
}
