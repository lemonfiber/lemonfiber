//! Generating a secret lemonfiber has to mint itself.
//!
//! Almost every credential in the stack is read, not made — a service generates
//! its own key and lemonfiber reads it. qBittorrent is the exception: it mints a
//! throwaway web UI password on each start and asks for it to be replaced, so
//! there is nothing durable to read and lemonfiber must supply one. This is where
//! that value is made.
//!
//! The randomness comes through the [`crate::ports::random::Random`] port, so the
//! rendering here — bytes to a recordable string — is tested against a fixed
//! sequence with a known result, while the operating system's CSPRNG stays in the
//! one adapter.

use crate::ports::random::Random;

/// How many random bytes back a generated secret.
///
/// Twenty-four bytes is 192 bits, far past any brute-force reach for a login
/// password, and a round number of bytes so the rendering has no remainder.
pub const SECRET_BYTES: usize = 24;

/// Generate a secret from the given source of randomness, or `None` where the
/// randomness could not be obtained — never a weaker fallback, because a
/// guessable password on the client the forwarded port authenticates to is the
/// failure this exists to prevent.
#[must_use]
pub fn generate(random: &dyn Random) -> Option<String> {
    random.bytes(SECRET_BYTES).map(|bytes| render(&bytes))
}

/// Render bytes as a lowercase-hex string: it uses only `0-9a-f`, so it is safe
/// to record as a `.env` value and read back without quoting or escaping, and it
/// carries two characters of the secret per byte with no encoding ambiguity.
#[must_use]
pub fn render(bytes: &[u8]) -> String {
    let mut rendered = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        rendered.push(hex_digit(byte >> 4));
        rendered.push(hex_digit(byte & 0x0f));
    }
    rendered
}

/// One nibble — a value 0 through 15 — as its lowercase hex character.
fn hex_digit(nibble: u8) -> char {
    match nibble {
        0..=9 => char::from(b'0' + nibble),
        _ => char::from(b'a' + nibble - 10),
    }
}

#[cfg(test)]
mod tests {
    use super::{generate, render, SECRET_BYTES};
    use crate::ports::random::Random;

    /// A randomness source that answers with exactly what a test scripts, so a
    /// generated secret has a known value to assert.
    struct Fixed(Option<Vec<u8>>);

    impl Random for Fixed {
        fn bytes(&self, _n: usize) -> Option<Vec<u8>> {
            self.0.clone()
        }
    }

    #[test]
    fn bytes_render_as_lowercase_hex_two_characters_each() {
        // 0x0a exercises both halves: the low nibble is a digit, the high a
        // letter after the split.
        assert_eq!(render(&[0x00, 0x0a, 0xff]), "000aff");
    }

    #[test]
    fn a_generated_secret_is_the_rendered_random_bytes() {
        let bytes = vec![0xde, 0xad, 0xbe, 0xef];
        let secret = generate(&Fixed(Some(bytes.clone())));
        assert_eq!(secret.as_deref(), Some("deadbeef"));
    }

    #[test]
    fn without_randomness_there_is_no_secret_rather_than_a_weak_one() {
        assert_eq!(generate(&Fixed(None)), None);
    }

    #[test]
    fn a_full_length_secret_is_two_hex_characters_per_byte() {
        let secret = generate(&Fixed(Some(vec![0x11; SECRET_BYTES])));
        assert_eq!(secret.map(|secret| secret.len()), Some(SECRET_BYTES * 2));
    }
}
