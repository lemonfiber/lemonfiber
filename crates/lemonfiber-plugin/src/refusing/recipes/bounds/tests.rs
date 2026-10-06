//! Guards that look back, retries within the published bounds, and captures that read a
//! place.

use crate::refusing::refusals;
use crate::schema::tests::WHOLE;
use crate::schema::Manifest;

/// What a manifest declaring only this recipe is refused for about its recipe.
fn refused(recipe: &str) -> Vec<String> {
    let before = WHOLE.split("[[recipe]]").next().unwrap_or_default();
    let text = format!(
        "{before}{recipe}\n[[secret]]\nid  = \"api-key\"\nof  = \"komga\"\nwhy = \"Held\"\n\n\
         [requires]\ncapabilities = [\"doctor.contribute\", \"recipe.run\"]\n"
    );
    Manifest::from_toml(&text).map_or_else(
        |refused| vec![refused.to_string()],
        |manifest| {
            refusals(&manifest, &["storage.hardlinks"])
                .iter()
                .map(ToString::to_string)
                .filter(|said| said.starts_with("recipe "))
                .collect()
        },
    )
}

/// Whether any refusal says every one of these words.
fn says(refused: &[String], words: &[&str]) -> bool {
    refused
        .iter()
        .any(|one| words.iter().all(|word| one.contains(word)))
}

/// A recipe of two steps to the plugin's own service, the second carrying `second`.
fn two(first: &str, second: &str) -> String {
    format!(
        r#"[[recipe]]
id    = "two"
title = "Two steps"
why   = "To see what each may say"

[[recipe.input]]
name   = "word"
origin = "operator"
ask    = "A word"

[[recipe.step]]
id   = "first"
call = {{ method = "GET", to = "komga", path = "/first" }}
{first}

[[recipe.step]]
id   = "second"
call = {{ method = "GET", to = "komga", path = "/second" }}
{second}
"#
    )
}

#[test]
fn a_guard_on_an_earlier_status_or_value_is_allowed() {
    for when in [
        r#"when = { step = "first", status = 404 }"#,
        r#"when = { value = "word", equals = "yes" }"#,
    ] {
        let said = refused(&two("", when));
        assert!(said.is_empty(), "{when}: {said:?}");
    }
}

#[test]
fn a_guard_looking_ahead_or_at_nothing_is_refused() {
    let ahead = refused(&two(r#"when = { step = "second", status = 200 }"#, ""));
    assert!(
        says(
            &ahead,
            &["step first.when", "second", "no step written before"]
        ),
        "{ahead:?}"
    );
    let itself = refused(&two("", r#"when = { step = "second", status = 200 }"#));
    assert!(
        says(&itself, &["step second.when", "no step written before"]),
        "{itself:?}"
    );
    let nothing = refused(&two("", r#"when = { value = "never", equals = "x" }"#));
    assert!(
        says(&nothing, &["never", "nothing before it"]),
        "{nothing:?}"
    );
}

#[test]
fn a_guard_written_as_neither_shape_is_refused() {
    for when in [
        "when = {}",
        r#"when = { step = "first" }"#,
        r#"when = { step = "first", status = 200, value = "word", equals = "x" }"#,
        r#"when = { value = "word" }"#,
    ] {
        let said = refused(&two("", when));
        assert!(
            says(&said, &["when", "neither of its shapes"]),
            "{when}: {said:?}"
        );
    }
}

#[test]
fn a_retry_within_the_bounds_is_allowed() {
    for retry in [
        r#"retry = { times = 10, every = "30s", until = { status = 200 } }"#,
        r#"retry = { times = 1, every = "1s", until = { value = "word", equals = "y" } }"#,
    ] {
        let said = refused(&two(retry, ""));
        assert!(said.is_empty(), "{retry}: {said:?}");
    }
}

/// A retry may wait on a value the step it retries captures.
#[test]
fn a_retry_may_end_on_a_value_its_own_step_captures() {
    let retry = "capture = [{ name = \"api-key\", from = \"token\", origin = \"stack-service\" }]\n\
                 retry = { times = 2, every = \"2s\", until = { value = \"api-key\", equals = \"x\" } }";
    let said = refused(&two(retry, ""));
    assert!(said.is_empty(), "{said:?}");
}

#[test]
fn a_retry_past_the_bounds_is_refused_naming_the_bound() {
    let many = refused(&two(
        r#"retry = { times = 11, every = "1s", until = { status = 200 } }"#,
        "",
    ));
    assert!(
        says(&many, &["retry.times", "11", "between 1 and 10"]),
        "{many:?}"
    );
    let none = refused(&two(
        r#"retry = { times = 0, every = "1s", until = { status = 200 } }"#,
        "",
    ));
    assert!(says(&none, &["retry.times", "0"]), "{none:?}");
    for every in ["31s", "0s", "5", "s", "5m", "1.5s", "-1s", "+5s"] {
        let said = refused(&two(
            &format!(r#"retry = {{ times = 1, every = "{every}", until = {{ status = 200 }} }}"#),
            "",
        ));
        assert!(
            says(&said, &["retry.every", "whole number of seconds"]),
            "{every}: {said:?}"
        );
    }
}

#[test]
fn a_retry_ending_on_neither_shape_or_on_nothing_is_refused() {
    let shaped = refused(&two(
        r#"retry = { times = 1, every = "1s", until = { step = "first", status = 200 } }"#,
        "",
    ));
    assert!(
        says(&shaped, &["retry.until", "neither of its shapes"]),
        "{shaped:?}"
    );
    let unknown = refused(&two(
        r#"retry = { times = 1, every = "1s", until = { value = "never", equals = "x" } }"#,
        "",
    ));
    assert!(says(&unknown, &["retry.until", "never"]), "{unknown:?}");
}

/// Every retry of a recipe together is held to the bound on all its waiting.
#[test]
fn a_recipe_whose_retries_could_wait_past_five_minutes_is_refused() {
    let retry = r#"retry = { times = 10, every = "16s", until = { status = 200 } }"#;
    let said = refused(&two(retry, retry));
    assert!(
        says(&said, &["recipe two", "320 seconds", "300"]),
        "{said:?}"
    );
    let within = r#"retry = { times = 10, every = "15s", until = { status = 200 } }"#;
    assert!(refused(&two(within, within)).is_empty());
}

#[test]
fn a_capture_reads_a_key_a_pointer_or_a_header() {
    for from in [
        "token",
        "/MediaContainer/Setting/[id=PlexOnlineToken]/value",
        "header.X-Plex-Token",
    ] {
        let capture = format!(
            "capture = [{{ name = \"api-key\", from = \"{from}\", origin = \"stack-service\" }}]"
        );
        let said = refused(&two(&capture, ""));
        assert!(said.is_empty(), "{from}: {said:?}");
    }
}

#[test]
fn a_capture_reading_no_place_is_refused() {
    for (from, why) in [
        ("header.", "not a header's name"),
        ("header.X Plex", "not a header's name"),
        ("/a/[b", "bracket"),
    ] {
        let capture = format!(
            "capture = [{{ name = \"api-key\", from = \"{from}\", origin = \"stack-service\" }}]"
        );
        let said = refused(&two(&capture, ""));
        assert!(
            says(&said, &["capture.from", "names no place", why]),
            "{from}: {said:?}"
        );
    }
}
