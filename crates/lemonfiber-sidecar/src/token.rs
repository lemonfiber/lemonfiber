//! A token as the files hold it, never the token itself.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

/// A token, as the files hold it: its SHA-256, in lowercase hexadecimal.
///
/// The token itself goes to whoever presents it and nowhere else, so a copy of a file
/// that holds its hash yields nothing that can be presented.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TokenHash(String);

impl TokenHash {
    /// The hash of `token`, as it is written down.
    #[must_use]
    pub fn of(token: &str) -> Self {
        let digest = ring::digest::digest(&ring::digest::SHA256, token.as_bytes());
        Self(hex(digest.as_ref()))
    }

    /// The hash as it is written down.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// `bytes` in lowercase hexadecimal.
fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut out, byte| {
            // Writing to a `String` cannot fail.
            let _ = write!(out, "{byte:02x}");
            out
        })
}
