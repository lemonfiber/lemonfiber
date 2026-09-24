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
pub(crate) const SECRET_BYTES: usize = 24;

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
mod tests;
