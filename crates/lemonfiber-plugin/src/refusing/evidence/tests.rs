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

/// A request naming a value its service makes afresh, by a plain pointer and a shape that
/// compiles, is refused nothing; one naming it any other way is refused by name.
#[test]
fn a_value_a_request_says_varies_is_named_by_a_plain_place_and_a_shape_that_compiles() {
    let varying = |varies: &str| {
        without(
            r#"request = { method = "GET", path = "/api/v1/series" }
expect  = { status = 401 }"#,
            &format!(
                "request = {{ method = \"GET\", path = \"/api/v1/series\", varies = {{ {varies} }} }}\nexpect  = {{ status = 401 }}"
            ),
        )
    };
    let said = varying(r#""/content/id" = "[0-9a-f]{40}""#);
    assert!(said.is_empty(), "{said:?}");
    for (varies, words) in [
        (r#"id = "[0-9a-f]{40}""#, "not a JSON Pointer"),
        (r#""/content/[id=x]" = "[0-9a-f]{40}""#, "picks a list"),
        (
            r#""/content/id" = "(""#,
            "not a pattern this build compiles",
        ),
    ] {
        let said = varying(varies);
        assert!(
            names(&said, &["request.varies", words]),
            "{varies} was not refused for {words}: {said:?}"
        );
    }
}

/// The proof's request in the fixture, sending `headers`.
fn proving_with(headers: &str) -> Vec<String> {
    without(
        r#"request = { method = "GET", path = "/actuator/health" }"#,
        &format!(
            "request = {{ method = \"GET\", path = \"/actuator/health\", headers = {{ {headers} }} }}"
        ),
    )
}

/// A proof may send a header from the closed list, holding a value shaped like no secret.
#[test]
fn a_proof_may_send_a_named_header() {
    for value in ["http://sonarr:8989/ping", "https://example.org/a/b?c=d"] {
        let said = proving_with(&format!("\"Target-URL\" = \"{value}\""));
        assert!(said.is_empty(), "{value}: {said:?}");
    }
}

/// Any other header, a header on a probe or a contribution, and a value shaped like a
/// secret are refused, each naming the field.
#[test]
fn a_header_off_the_list_off_a_proof_or_shaped_like_a_secret_is_refused_by_name() {
    for (headers, words) in [
        ("\"X-Api-Key\" = \"x\"", "not a header a proof may send"),
        (
            "Authorization = \"Basic x\"",
            "not a header a proof may send",
        ),
        (
            "\"target-url\" = \"http://sonarr:8989/ping\"",
            "not a header a proof may send",
        ),
        (
            "\"Target-URL\" = \"http://user:pass@sonarr:8989/ping\"",
            "shaped like a secret",
        ),
        (
            "\"Target-URL\" = \"http://sonarr:8989/ping?apikey=0123456789abcdef0123\"",
            "shaped like a secret",
        ),
    ] {
        let said = proving_with(headers);
        assert!(
            names(&said, &["proof komga.serves.request.headers", words]),
            "{headers} was not refused for {words}: {said:?}"
        );
    }
    for (was, now) in [
        (
            r#"id      = "guarded"
request = { method = "GET", path = "/api/v1/series" }"#,
            r#"id      = "guarded"
request = { method = "GET", path = "/api/v1/series", headers = { "Target-URL" = "http://sonarr:8989/ping" } }"#,
        ),
        (
            r#"request   = { method = "GET", path = "/api/v1/claim" }"#,
            r#"request   = { method = "GET", path = "/api/v1/claim", headers = { "Target-URL" = "http://sonarr:8989/ping" } }"#,
        ),
    ] {
        let said = without(was, now);
        assert!(
            names(&said, &["request.headers", "only a proof sends one"]),
            "{now}: {said:?}"
        );
    }
}

/// What a value has to hold to be read as a secret, and what it may hold and not be.
#[test]
fn a_value_is_shaped_like_a_secret_by_user_information_or_a_long_mixed_run() {
    for (case, secret) in [
        "https://ana:hunter2@example.org/",
        "ghp_0123456789abcdefABCDEF",
        "Bearer YWxhZGRpbjpvcGVuc2VzYW1lMTIzNA==",
    ]
    .into_iter()
    .enumerate()
    {
        assert!(super::secret_shaped(secret), "secret case {case}");
    }
    for plain in [
        "http://sonarr:8989/ping",
        "https://example.org/a@b",
        "abcdefghijklmnopqrstuvwxyz",
        "01234567890123456789012",
        "mailto:ana@example.org",
    ] {
        assert!(!super::secret_shaped(plain), "{plain}");
    }
}
