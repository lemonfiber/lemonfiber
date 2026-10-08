use super::{render_json, render_reference, CODES_JSON, CODES_PATH};
use serde_json::Value;
use std::path::Path;

fn committed(path: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read_to_string(root.join(path)).unwrap_or_default()
}

#[test]
fn the_committed_registry_still_matches_the_codes_the_crates_declare() {
    assert_eq!(
        committed(CODES_JSON),
        render_json(),
        "contract/codes.json is out of date — regenerate it with `just codes`"
    );
}

#[test]
fn the_committed_reference_is_the_one_the_committed_registry_renders() {
    assert_eq!(
        Ok(committed(CODES_PATH)),
        render_reference(&committed(CODES_JSON)),
        "the error-code reference is out of date — regenerate it with `just codes`"
    );
}

#[test]
fn every_code_is_published_with_all_it_is_declared_with() {
    let published: Value = serde_json::from_str(&render_json()).unwrap_or_default();
    let codes = published
        .get("codes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert!(!codes.is_empty());
    for code in &codes {
        for key in [
            "code", "family", "name", "severity", "summary", "meaning", "remedy", "since",
        ] {
            assert!(
                code.get(key)
                    .and_then(Value::as_str)
                    .is_some_and(|text| !text.is_empty()),
                "{code} has no {key}"
            );
        }
        assert!(
            code.get("exit")
                .and_then(Value::as_u64)
                .is_some_and(|exit| exit > 0),
            "{code}"
        );
        assert!(
            code.get("status")
                .and_then(Value::as_u64)
                .is_some_and(|status| (400..600).contains(&status)),
            "{code}"
        );
    }
}

#[test]
fn a_code_carries_its_family_exit_and_status_as_declared() {
    let published: Value = serde_json::from_str(&render_json()).unwrap_or_default();
    let stack = published
        .get("codes")
        .and_then(Value::as_array)
        .and_then(|codes| {
            codes
                .iter()
                .find(|code| code.get("code").and_then(Value::as_str) == Some("STACK-10"))
        })
        .cloned()
        .unwrap_or_default();
    let field = |key: &str| stack.get(key).cloned().unwrap_or_default();
    assert_eq!(field("family"), "STACK");
    assert_eq!(field("name"), "STACK_UNASSEMBLED");
    assert_eq!(field("exit"), 5);
    assert_eq!(field("severity"), "error");
}

#[test]
fn a_registry_that_is_not_json_is_refused_by_name() {
    assert!(render_reference("not json").is_err());
}

#[test]
fn a_registry_missing_a_list_or_a_field_is_refused_by_name() {
    assert_eq!(render_reference("{}"), Err("no families list".to_owned()));
    assert_eq!(
        render_reference(r#"{"families":[{"prefix":"ACK"}],"codes":[],"retired":[]}"#),
        Err("an entry has no covers".to_owned())
    );
    assert_eq!(
        render_reference(r#"{"families":[{"prefix":"ACK","covers":"x"}],"retired":[]}"#),
        Err("no codes list".to_owned())
    );
}

#[test]
fn a_code_of_another_family_is_not_listed_under_this_one() {
    let rendered = render_reference(
        r#"{"families":[{"prefix":"ACK","covers":"answering"}],
            "codes":[{"family":"ASK","code":"ASK-1"}],"retired":["QUOTA-3"]}"#,
    );
    assert!(rendered.is_ok_and(|text| !text.contains("ASK-1") && text.contains("`QUOTA-3`")));
}

/// Written an entry to a line, so the registry stays within what a reader takes in and
/// is still the JSON a client parses.
#[test]
fn the_registry_is_an_entry_to_a_line_and_parses() {
    let rendered = render_json();
    let published: Value = serde_json::from_str(&rendered).unwrap_or_default();
    let codes = published
        .get("codes")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    assert!(codes > 300);
    assert!(rendered.lines().count() < 1000);
}
