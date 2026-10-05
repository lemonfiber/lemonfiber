use super::{key, KEY_LIMIT};

/// A key spelled the way services spell theirs is taken; anything a container could
/// have written instead of one is not.
#[test]
fn only_a_key_spelled_as_a_generated_key_is_taken() {
    for made in [
        "0123456789abcdefABCDEF0123456789",
        "MTcwMDAwMDAwMDAwMGFiYw==",
        "2f0d8e9c-0a4b-4f6e-9d3a-5b7c1e2f3a4b",
        "a_b.c+d/e",
    ] {
        assert_eq!(key(made).as_deref(), Some(made));
    }
    for planted in [
        "${WIREGUARD_PRIVATE_KEY}",
        "$OTHER",
        "it's",
        "say\"hi",
        "two words",
        "line\nbreak",
        "café☃clé",
        "",
        &"a".repeat(KEY_LIMIT + 1),
    ] {
        assert_eq!(key(planted), None, "{planted:?}");
    }
    assert!(key(&"a".repeat(KEY_LIMIT)).is_some());
}
