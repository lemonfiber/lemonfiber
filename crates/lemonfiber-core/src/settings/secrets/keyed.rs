//! A secret's fingerprint as the journal records it: an HMAC-SHA256 of its value under a
//! random key only the secret store holds, so a journal that travels without the store
//! says nothing about any value, and one beside it still tells two values apart.

use ring::hmac;

use crate::ports::random::Random;

/// What a keyed fingerprint is written behind, apart from the unkeyed one a contract
/// answers with.
pub(crate) const MARK: &str = "#";

/// What stands for a fingerprint an earlier release recorded that no value at hand could
/// be keyed from: no value's fingerprint matches it.
pub(crate) const UNMATCHED: &str = "#-";

/// Bytes of key, drawn from the randomness port when the store first needs one.
const KEY_BYTES: usize = 32;

/// Hex digits of the MAC a fingerprint keeps: sixty-four bits, so two values the store
/// holds are told apart.
const SHOWN: usize = 16;

/// A key for a store that has none, or nothing where this machine would not supply one.
pub(super) fn minted(random: &dyn Random) -> Option<String> {
    random
        .bytes(KEY_BYTES)
        .filter(|bytes| bytes.len() == KEY_BYTES)
        .map(|bytes| crate::secret::render(&bytes))
}

/// The fingerprint of `value` under `key`.
pub(super) fn fingerprint(key: &str, value: &str) -> String {
    let tag = hmac::sign(
        &hmac::Key::new(hmac::HMAC_SHA256, key.as_bytes()),
        value.as_bytes(),
    );
    let mut shown = crate::secret::render(tag.as_ref());
    shown.truncate(SHOWN);
    format!("{MARK}{shown}")
}

/// Whether `fingerprint` is keyed, rather than one an earlier release recorded unkeyed.
pub(crate) fn is_keyed(fingerprint: &str) -> bool {
    fingerprint.starts_with(MARK)
}

#[cfg(test)]
mod tests;
