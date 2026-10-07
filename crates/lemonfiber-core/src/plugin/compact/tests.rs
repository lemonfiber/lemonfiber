use super::collapsed;

/// What `serde_json` writes, which is what this is always handed.
fn pretty(value: &serde_json::Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_default() + "\n"
}

#[test]
fn a_list_or_object_of_scalars_is_one_line_and_nesting_keeps_its_indent() {
    let value = serde_json::json!({
        "a": [1, "two", null, true],
        "b": [{"c": []}],
        "d": {},
        "e": {"f": "g", "h": 1}
    });

    assert_eq!(
        collapsed(&pretty(&value)).as_deref(),
        Some(
            "{\n  \"a\": [1, \"two\", null, true],\n  \"b\": [\n    {\n      \"c\": []\n    }\n  ],\n  \
             \"d\": {},\n  \"e\": {\"f\": \"g\", \"h\": 1}\n}\n"
        )
    );
}

#[test]
fn an_object_holding_a_list_beside_a_scalar_keeps_its_indent() {
    let value = serde_json::json!({ "n": 1, "list": [1, 2] });

    assert_eq!(
        collapsed(&pretty(&value)).as_deref(),
        Some("{\n  \"list\": [1, 2],\n  \"n\": 1\n}\n")
    );
}

#[test]
fn brackets_inside_strings_are_text_not_structure() {
    let value = serde_json::json!({ "said": ["ends with {", "} starts", "[x]"] });

    assert_eq!(
        collapsed(&pretty(&value)).as_deref(),
        Some("{\n  \"said\": [\"ends with {\", \"} starts\", \"[x]\"]\n}\n")
    );
}

#[test]
fn what_is_written_reads_back_as_the_same_value_and_is_written_the_same_again() {
    let value = serde_json::json!({ "x": [[1, 2], {"y": [3]}], "z": "w" });
    let once = collapsed(&pretty(&value)).unwrap_or_default();
    let read: serde_json::Value = serde_json::from_str(&once).unwrap_or_default();

    assert_eq!(read, value);
    assert_eq!(collapsed(&pretty(&read)).as_deref(), Some(once.as_str()));
}

#[test]
fn a_document_that_is_not_json_is_refused() {
    assert_eq!(collapsed("{\n  \"a\": [\n"), None);
    assert_eq!(collapsed("}\n"), None);
    assert_eq!(collapsed("not json\n"), None);
}
