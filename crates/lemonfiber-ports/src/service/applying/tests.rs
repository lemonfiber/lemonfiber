use super::*;

#[test]
fn in_force_at_once_crosses_as_null_and_on_restart_as_its_word() {
    assert_eq!(
        serde_json::to_string(&Applied::Now).ok().as_deref(),
        Some("null")
    );
    assert_eq!(
        serde_json::to_string(&Applied::OnRestart).ok().as_deref(),
        Some("\"on-restart\"")
    );
    assert_eq!(
        serde_json::from_str::<Applied>("null").ok(),
        Some(Applied::Now)
    );
    assert_eq!(
        serde_json::from_str::<Applied>("\"on-restart\"").ok(),
        Some(Applied::OnRestart)
    );
    assert!(serde_json::from_str::<Applied>("\"later\"").is_err());
}

#[test]
fn its_schema_is_the_word_or_nothing() {
    let schema = serde_json::to_string(&schemars::schema_for!(Applied)).unwrap_or_default();
    assert!(schema.contains("on-restart"), "{schema}");
    assert!(schema.contains("null"), "{schema}");
}
