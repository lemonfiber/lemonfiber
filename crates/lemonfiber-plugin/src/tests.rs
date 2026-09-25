use super::{is_compatible, SCHEMA_VERSION, SUPPORTED_SCHEMA_VERSIONS};

#[test]
fn reads_its_own_schema_version() {
    assert!(is_compatible(SCHEMA_VERSION));
}

#[test]
fn refuses_a_version_it_does_not_support() {
    assert!(!is_compatible(0));
    assert!(!is_compatible(SCHEMA_VERSION + 1));
}

#[test]
fn supports_every_generation_it_has_ever_published() {
    assert!(SUPPORTED_SCHEMA_VERSIONS.contains(&SCHEMA_VERSION));
}
