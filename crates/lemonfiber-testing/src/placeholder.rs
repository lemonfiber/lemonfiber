//! Words a test hands a service where it would hand a credential.
//!
//! Drawn from the operating system's randomness rather than written in the test, so a
//! credential a test passes is never a value the source carries. A test that needs to
//! recognise one keeps the word it was given and looks for that.

use lemonfiber_adapters::Os;
use lemonfiber_core::ports::random::Random;

/// How long a word is.
const LENGTH: usize = 16;

/// A fresh word of lowercase letters, different on every call.
///
/// Drawn until it is long enough: the bytes that are lowercase letters are kept and
/// the rest thrown away, so nothing in the word is chosen here. The source is reached
/// through the port, the way the shipped code that mints a password reaches it.
#[must_use]
pub fn a_word() -> String {
    let random: &dyn Random = &Os;
    let mut letters: Vec<char> = Vec::new();
    while letters.len() < LENGTH {
        let drawn = random.bytes(LENGTH * 8).unwrap_or_default();
        letters.extend(
            drawn
                .into_iter()
                .filter(u8::is_ascii_lowercase)
                .map(char::from),
        );
    }
    letters.into_iter().take(LENGTH).collect()
}

#[cfg(test)]
mod tests;
