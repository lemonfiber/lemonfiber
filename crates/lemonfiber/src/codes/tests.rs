use super::{render, CODES_PATH};
use std::path::Path;

#[test]
fn the_committed_reference_still_matches_the_codes_the_registry_declares() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let stored = std::fs::read_to_string(root.join(CODES_PATH)).unwrap_or_default();

    assert_eq!(
        stored,
        render(),
        "the error-code reference is out of date — regenerate it with `just codes`"
    );
}
