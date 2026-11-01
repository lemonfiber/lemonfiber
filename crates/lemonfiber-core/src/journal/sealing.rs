//! A credential in the change journal, as it is kept on disk: by its fingerprint, never
//! by its value.
//!
//! Every credential lemonfiber holds is a secret in the secret store, and the store keeps
//! each one's one earlier value, so putting a change to one back needs only to know that
//! the store still holds what the change left: the fingerprints of the value before and
//! after are enough, and a journal that held the value would be a second copy of every
//! credential, kept long after the account it belonged to was rotated away.
//!
//! A fingerprint is keyed, an HMAC-SHA256 under a random key only the secret store holds,
//! so a journal that travels without the store says nothing about any value.
//!
//! An earlier release kept a credential's value in the journal sealed, under a key kept
//! beside it, or fingerprinted a secret unkeyed. Such a journal still reads entry for
//! entry: a sealed value opens where the key is there, and the first time the journal is
//! written again every credential's value and unkeyed fingerprint in it is replaced by its
//! keyed fingerprint, so no journal holds one after that. One the store cannot key is
//! written as a mark no value matches, so undoing it says the secret must be replaced at
//! the terminal. A setting an earlier release sealed only for a name that read like a
//! credential is opened and written back as the setting it is.
//!
//! **XChaCha20-Poly1305** is what those values were sealed with: authenticated, so a
//! record somebody edited does not open rather than opening to something else.

use std::path::{Path, PathBuf};

use chacha20poly1305::aead::{Aead as _, KeyInit as _};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};

use crate::settings::legacy::{held_as, source_of};
use crate::settings::{Secrets, UNMATCHED};

use super::{Change, Kind};

/// The file an earlier release kept the journal's key in, beside the journal itself.
pub const KEY_FILE: &str = "journal.key";

/// What a sealed value is written behind, so a reader can tell one from a value.
const MARKER: &str = "sealed:1:";

/// Bytes of nonce a sealed value opens with.
const NONCE_BYTES: usize = 24;

/// The key an earlier release sealed one journal's credentials under.
pub struct Seal {
    /// The key, where this machine has one that reads.
    key: Option<Key>,
}

impl Seal {
    /// The key kept beside `journal`, opening nothing where there is none.
    #[must_use]
    pub fn kept(journal: &Path) -> Self {
        Self {
            key: std::fs::read_to_string(beside(journal))
                .ok()
                .and_then(|text| unhex(text.trim()))
                .and_then(|bytes| Key::try_from(bytes.as_slice()).ok()),
        }
    }

    /// Take the key away from beside `journal`, where one reads there: once the journal
    /// is written again every value it opens has been opened, so it opens nothing more.
    ///
    /// Best effort: a key left behind opens only what was already written back opened.
    pub fn retired(journal: &Path) {
        if Self::kept(journal).key.is_some() {
            let _ = std::fs::remove_file(beside(journal));
        }
    }

    /// The change as it was made, where this key opens what was sealed.
    ///
    /// A value that will not open is left as it was read, still marked, because the two
    /// reasons it would not, no key or a record somebody edited, are both reasons to
    /// refuse to act on it rather than reasons to drop the entry.
    #[must_use]
    pub fn opening(&self, change: &Change) -> Change {
        let Kind::Set {
            key,
            previous,
            current,
        } = &change.kind
        else {
            return change.clone();
        };
        let opened = |value: &str| self.open(value).unwrap_or_else(|| value.to_owned());
        Change {
            kind: Kind::Set {
                key: key.clone(),
                previous: previous.as_deref().map(opened),
                current: opened(current),
            },
            ..change.clone()
        }
    }

    /// One value opened, or nothing where it is not sealed, not sealed under this key,
    /// or not what it was when it was written.
    fn open(&self, sealed: &str) -> Option<String> {
        let key = self.key.as_ref()?;
        let body = unhex(sealed.strip_prefix(MARKER)?)?;
        let nonce = body
            .get(..NONCE_BYTES)
            .and_then(|bytes| XNonce::try_from(bytes).ok())?;
        let rest = body.get(NONCE_BYTES..)?;
        let clear = XChaCha20Poly1305::new(key).decrypt(&nonce, rest).ok()?;
        String::from_utf8(clear).ok()
    }
}

/// The secret store beside the journal at `journal`, whose key its fingerprints are
/// taken under.
///
/// # Errors
///
/// Where the store is there and cannot be read: a journal written without it would mark
/// every fingerprint it could not key, which no later read could undo.
pub(crate) fn store_of(journal: &Path) -> Result<Secrets, crate::config::store::Failure> {
    Secrets::at(&journal.with_file_name(crate::config::paths::SECRETS))
}

/// The change as it is written down beside the secret store `store`: a credential set in
/// the environment file recorded as a change to the secret the store holds it as, and a
/// change to a secret an earlier release fingerprinted unkeyed, each by the fingerprints
/// the store's key gives its values; every other change exactly as it arrived.
///
/// A value or an unkeyed fingerprint the store cannot key, because it has no key or holds
/// no value the fingerprint is of, is written as the mark no value matches.
#[must_use]
pub(crate) fn scrubbed(change: &Change, store: &Secrets) -> Change {
    let keyed = |value: &str| store.keyed(value).unwrap_or_else(|| UNMATCHED.to_owned());
    let kind = match &change.kind {
        Kind::Set {
            key,
            previous,
            current,
        } => {
            let Some(name) = held_as(key) else {
                return change.clone();
            };
            Kind::Secret {
                name,
                from: source_of(key).as_str().to_owned(),
                previous: previous.as_deref().map(keyed),
                current: Some(keyed(current)),
                filed: None,
            }
        }
        Kind::Secret {
            name,
            from,
            previous,
            current,
            filed,
        } => Kind::Secret {
            name: name.clone(),
            from: from.clone(),
            previous: previous.as_deref().map(|held| store.rekeyed(name, held)),
            current: current.as_deref().map(|held| store.rekeyed(name, held)),
            filed: filed.clone(),
        },
        _ => return change.clone(),
    };
    Change {
        kind,
        ..change.clone()
    }
}

/// Whether a value is written the way a sealed one is, and so is not the value.
///
/// Asked by everything that would otherwise act on what it read: a reversal that wrote
/// this into the environment file would report a setting restored and leave the operator
/// authenticating with a line of hexadecimal.
#[must_use]
pub fn is_sealed(value: &str) -> bool {
    value.starts_with(MARKER)
}

/// Where the key for the journal at `journal` lives.
fn beside(journal: &Path) -> PathBuf {
    journal.with_file_name(KEY_FILE)
}

/// The bytes a run of hexadecimal stands for, or nothing where it is not one.
fn unhex(text: &str) -> Option<Vec<u8>> {
    let digits = text.as_bytes();
    if !digits.len().is_multiple_of(2) {
        return None;
    }
    digits
        .chunks(2)
        .map(|pair| {
            std::str::from_utf8(pair)
                .ok()
                .filter(|two| two.chars().all(|digit| digit.is_ascii_hexdigit()))
                .and_then(|two| u8::from_str_radix(two, 16).ok())
        })
        .collect()
}

#[cfg(test)]
mod tests;
