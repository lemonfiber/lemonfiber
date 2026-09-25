use super::{Row, OURS};

/// A row the service ships is never this program's, whatever its search says.
#[test]
fn a_built_in_row_is_never_ours() {
    let row = Row {
        id: 1,
        kind: 1,
        is_built_in: true,
        enabled: true,
        title: None,
        data: Some(OURS.to_owned()),
    };

    assert!(
        !row.ours(),
        "a row the service ships was claimed by this program because of a field an \
         operator could have typed"
    );
}

/// A row somebody else added is left alone.
#[test]
fn a_row_somebody_else_added_is_not_ours() {
    let row = Row {
        id: 2,
        kind: 17,
        is_built_in: false,
        enabled: true,
        title: Some("Wessel's own row".to_owned()),
        data: Some("213".to_owned()),
    };

    assert!(
        !row.ours(),
        "somebody else's row was claimed by this program"
    );
}

/// A row with no heading reads as an empty sentence rather than as no row.
#[test]
fn a_row_with_no_heading_is_an_empty_sentence() {
    let row = Row {
        id: 3,
        kind: 17,
        is_built_in: false,
        enabled: true,
        title: None,
        data: Some(OURS.to_owned()),
    };

    assert!(row.ours());
    assert_eq!(row.sentence(), String::new());
}

/// What is written back names every field the write assigns.
#[test]
fn what_is_written_back_names_every_field_the_write_assigns() {
    let row = Row {
        id: 7,
        kind: 17,
        is_built_in: false,
        enabled: false,
        title: Some("a notice".to_owned()),
        data: Some(OURS.to_owned()),
    };

    let written = row.written(3, true);

    // The whole document rather than field by field: what the service is sent
    // is the shape as well as the values, and a field that should not be there
    // is as wrong as one that is missing.
    assert_eq!(
        written,
        serde_json::json!({
            "id": 7,
            "type": 17,
            "isBuiltIn": false,
            "enabled": true,
            "order": 3,
            "title": "a notice",
            "data": OURS,
        })
    );
}
