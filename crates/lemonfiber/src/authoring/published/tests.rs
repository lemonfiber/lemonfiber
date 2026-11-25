use super::Row;

#[test]
fn a_rows_bounds_and_closed_sets_keep_the_order_they_are_published_in() {
    let row: Result<Row, _> = serde_json::from_str(
        r#"{"required": ["id"], "optional": [],
            "bounds": {"timeout_s": {"min": 1, "max": 30, "default": 10},
                       "attempts": {"min": 0, "max": 3, "default": 1}},
            "enums": {"category": ["vpn", "storage"], "after": ["a"]}}"#,
    );

    let row = row.ok();
    let bounded: Vec<&str> = row
        .iter()
        .flat_map(|row| row.bounds.iter().map(|(field, _)| field.as_str()))
        .collect();
    let closed: Vec<&str> = row
        .iter()
        .flat_map(|row| row.enums.iter().map(|(field, _)| field.as_str()))
        .collect();
    assert_eq!(bounded, ["timeout_s", "attempts"]);
    assert_eq!(closed, ["category", "after"]);
}

#[test]
fn a_row_whose_bounds_are_no_table_is_not_read() {
    let refused =
        serde_json::from_str::<Row>(r#"{"required": [], "optional": [], "bounds": ["timeout_s"]}"#)
            .err()
            .map(|fault| fault.to_string());

    assert!(
        refused
            .as_deref()
            .is_some_and(|said| said.contains("a table keyed by field")),
        "{refused:?}"
    );
}
