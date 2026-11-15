use super::{generate, render, SECRET_BYTES};
use lemonfiber_fixtures::ports::Chance;

#[test]
fn bytes_render_as_lowercase_hex_two_characters_each() {
    // 0x0a exercises both halves: the low nibble is a digit, the high a
    // letter after the split.
    assert_eq!(render(&[0x00, 0x0a, 0xff]), "000aff");
}

#[test]
fn a_generated_secret_is_the_rendered_random_bytes() {
    let bytes = vec![0xde, 0xad, 0xbe, 0xef];
    let secret = generate(&Chance::exactly(Some(bytes.clone())));
    assert_eq!(secret.as_deref(), Some("deadbeef"));
}

#[test]
fn without_randomness_there_is_no_secret_rather_than_a_weak_one() {
    assert_eq!(generate(&Chance::exactly(None)), None);
}

#[test]
fn a_full_length_secret_is_two_hex_characters_per_byte() {
    let secret = generate(&Chance::exactly(Some(vec![0x11; SECRET_BYTES])));
    assert_eq!(secret.map(|secret| secret.len()), Some(SECRET_BYTES * 2));
}
