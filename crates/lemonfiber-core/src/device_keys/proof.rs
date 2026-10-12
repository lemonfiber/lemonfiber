//! What a device key signs to prove the operator for one mint.
//!
//! The challenge and every part of the mint, one to a line, so a signature made for one
//! mint proves no other: a different name, scope, purpose or kind is a different text,
//! and a challenge is good for one request.

use super::{decoded, PublicKey};

/// The first line of every text a device key signs, naming its form.
pub const FORM: &str = "lemonfiber-mint/1";

/// What ends every line.
pub const LINE_END: char = '\n';

/// The last line of a mint asked for as a claim.
pub const CLAIM: &str = "claim";

/// The last line of a mint that hands over the key's secret.
pub const KEY: &str = "key";

/// The mint a signature is made for, as the request asking for it spells each part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mint<'a> {
    /// The key's name.
    pub name: &'a str,
    /// Its scope, as the request wrote it.
    pub scope: &'a str,
    /// Its purpose, as the request wrote it.
    pub purpose: &'a str,
    /// Whether it is asked for as a claim.
    pub claim: bool,
}

impl Mint<'_> {
    /// The text a device key signs for this mint under `challenge`: [`FORM`], the
    /// challenge, the name, the scope, the purpose and [`CLAIM`] or [`KEY`], each ended
    /// by [`LINE_END`], in UTF-8.
    #[must_use]
    pub fn signed(&self, challenge: &str) -> String {
        let kind = if self.claim { CLAIM } else { KEY };
        [FORM, challenge, self.name, self.scope, self.purpose, kind]
            .iter()
            .fold(String::new(), |mut text, line| {
                text.push_str(line);
                text.push(LINE_END);
                text
            })
    }

    /// Whether `signature`, ECDSA in DER and base64url, is `key`'s over this mint under
    /// `challenge`.
    ///
    /// A line holding a line end would let one text read as another, so a mint any of
    /// whose parts holds one is proved by nothing.
    #[must_use]
    pub fn proved(&self, challenge: &str, signature: &str, key: &PublicKey) -> bool {
        let one_line = [challenge, self.name, self.scope, self.purpose]
            .iter()
            .all(|part| !part.contains(LINE_END));
        let Some(signature) = decoded(signature) else {
            return false;
        };
        one_line && key.signed(self.signed(challenge).as_bytes(), &signature)
    }
}

#[cfg(test)]
mod tests;
