//! A value's SHA-256, as a contract answers it in place of a credential it was applied.

use std::fmt::Write as _;

/// The SHA-256 of `value`, in lower-case hexadecimal.
#[must_use]
pub fn sha256(value: &str) -> String {
    hex(ring::digest::digest(&ring::digest::SHA256, value.as_bytes()).as_ref())
}

/// `bytes` in lower-case hexadecimal, two digits to a byte.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(
        String::with_capacity(bytes.len() * 2),
        |mut written, byte| {
            let _ = write!(written, "{byte:02x}");
            written
        },
    )
}

#[cfg(test)]
mod tests;
