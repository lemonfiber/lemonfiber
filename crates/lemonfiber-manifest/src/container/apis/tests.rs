use super::Apis;

#[test]
fn one_api_is_written_as_a_string_and_more_as_a_list() {
    let one: Result<Apis, _> = serde_json::from_str(r#""jellyfin""#);
    let many: Result<Apis, _> = serde_json::from_str(r#"["jellyfin","plex"]"#);
    assert_eq!(one.as_ref().ok(), Some(&Apis::one("jellyfin")));
    assert_eq!(
        many.as_ref().ok().map(Apis::names),
        Some(&["jellyfin".to_owned(), "plex".to_owned()][..])
    );
    assert_eq!(
        one.ok()
            .and_then(|apis| serde_json::to_string(&apis).ok())
            .as_deref(),
        Some(r#""jellyfin""#)
    );
    let written = many.ok();
    assert_eq!(
        written
            .as_ref()
            .and_then(|apis| serde_json::to_string(apis).ok())
            .as_deref(),
        Some(r#"["jellyfin","plex"]"#)
    );
    assert!(written.as_ref().is_some_and(|apis| apis.names_any("plex")));
    assert!(!written.as_ref().is_some_and(|apis| apis.names_any("emby")));
    assert_eq!(
        written.map(|apis| apis.said()).as_deref(),
        Some("jellyfin or plex")
    );
}

#[test]
fn either_shape_is_described() {
    let schema = serde_json::to_value(schemars::schema_for!(Apis)).unwrap_or_default();
    let shapes = schema
        .get("anyOf")
        .and_then(serde_json::Value::as_array)
        .map(Vec::len);
    assert_eq!(shapes, Some(2), "{schema}");
}
