//! The credentials a change journal holds, as they are kept on disk.
//!
//! Putting a change back writes what the setting held before it, so the record has to
//! keep that value — and some of these settings hold the indexer key and the Usenet
//! password. That makes the journal the one file here that keeps a credential nothing
//! reads: the environment file holds today's value because the stack needs it, while the
//! journal holds every value each of those settings has ever held, long after the account
//! one of them belonged to was rotated away.
//!
//! Sealed rather than left out, because leaving it out is a reversal that cannot reverse.
//! An operator who undoes a change and is handed the setting back empty, with the
//! credential it replaced recorded nowhere, has been given the shape of an undo without
//! the thing itself. Sealed rather than left in clear, because a credential that goes on
//! sitting in a file after it was replaced has not been rotated away from anything.
//!
//! **What this is not.** The key sits beside the journal, in the same directory, owned by
//! the same user, and anything running as the operator reads both. That is the sentence
//! the credential inventory already says about the environment file, and it is no less
//! true here; this does not make the journal secret from this machine, and a product that
//! implied it did would be trading a real guarantee for a longer word. What it does make
//! the journal is no longer a second copy of a credential that travels on its own — a
//! line pasted into a forum thread, a `journal.jsonl` lifted out of a directory, a backup
//! that took the records and left the key. Those are how a file leaks, and a sealed value
//! survives all three.
//!
//! **XChaCha20-Poly1305**, with a fresh nonce for every value. Authenticated, so a record
//! somebody edited is a record that will not open rather than one that opens to something
//! else; extended-nonce, so nonces may be drawn at random without anybody having to count
//! how many values one journal has held. Both the key and each nonce come through the
//! randomness port every other secret here comes through, so this is exercised against
//! bytes a test chose and the operating system's own source stays in its one adapter.
//!
//! Only a setting whose *name* says it holds a credential is sealed, which is the same
//! question `history` asks before it declines to print one. A field changed inside a
//! service is not sealed: the one thing that records those is the download-client
//! category, and a rule with nothing behind it is a rule nobody maintains.
//!
//! A value that will not open is handed back exactly as it was read, marker and all —
//! and written back that way too, so a rewrite of the file does not bury it under a
//! second key that a restored one could no longer get past. Every reader that could act
//! on such a value asks before it does. That is the state a journal restored without its
//! key is in, and the failure worth arranging against is not that such a value cannot be
//! put back: it is a reversal that quietly writes a line of ciphertext into the
//! environment file and reports the setting restored.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use chacha20poly1305::aead::{Aead as _, KeyInit as _};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};

use crate::config::store::{self, is_secret};
use crate::ports::random::Random;

use super::{Change, Kind};

/// The file the journal's key is kept in, beside the journal itself.
///
/// Beside it rather than inside it: a key in the file it opens is not a key. Named here
/// as well as in the layout, because both readers of the journal are handed its path and
/// neither is handed the layout it came from.
pub const KEY_FILE: &str = "journal.key";

/// What a sealed value is written behind, so a reader can tell one from a value.
///
/// Carries its version, because the answer to "raise the parameters later" has to be a
/// second marker rather than a migration — a journal is read years after it was written,
/// and every entry in one may have been written by a different version of this. A second
/// mark is taught to [`is_sealed`] as well as to [`Seal::open`]: a reader that could not
/// tell a later version's record from a value would write one into the settings file.
const MARKER: &str = "sealed:1:";

/// Bytes of key, which is what XChaCha20-Poly1305 takes.
const KEY_BYTES: usize = 32;

/// Bytes of nonce. The extended one: at 24 bytes a value drawn at random is unique for
/// every journal anybody will ever write, without a counter to keep.
const NONCE_BYTES: usize = 24;

/// The key one journal's credentials are sealed under.
///
/// Always a value, never an absence, and that is deliberate: a caller that had to hold
/// an `Option` would have somewhere to decide, for itself, what to do without a key —
/// and the answer must be the same everywhere, which is that a value that cannot be
/// sealed is not written and a value that cannot be opened is not acted on.
pub struct Seal {
    /// The key, where this machine has one that reads.
    key: Option<Key>,
}

impl Seal {
    /// The key already kept beside `journal`, opening nothing where there is none.
    ///
    /// The reading half. A read never mints a key: a journal written in clear by an
    /// older version still reads entry for entry, and one whose key has been lost says
    /// so rather than quietly starting a new one that opens none of it.
    #[must_use]
    pub fn kept(journal: &Path) -> Self {
        Self {
            key: std::fs::read_to_string(beside(journal))
                .ok()
                .and_then(|text| unhex(text.trim()))
                .and_then(|bytes| Key::try_from(bytes.as_slice()).ok()),
        }
    }

    /// The key kept beside `journal`, made and written down where there is none yet.
    ///
    /// The writing half, and the only thing that ever creates a key. Written through the
    /// same seam every other small file here goes through, so it lands owner-only in an
    /// owner-only directory. A machine whose randomness will not answer gets a seal that
    /// seals nothing, which is what stops a credential being written in clear because
    /// the alternative failed.
    #[must_use]
    pub fn minted(journal: &Path, random: &dyn Random) -> Self {
        let kept = Self::kept(journal);
        // A key already there is never replaced, and that includes one that will not
        // read. Writing a fresh key over it would orphan every value already sealed
        // under it — and a file that is briefly unreadable is a likelier explanation
        // than a key that has genuinely gone, so the recoverable reading is the one
        // this takes.
        if kept.key.is_some() || beside(journal).exists() {
            return kept;
        }
        let made = random
            .bytes(KEY_BYTES)
            .and_then(|bytes| Key::try_from(bytes.as_slice()).ok().map(|key| (bytes, key)));
        let Some((bytes, key)) = made else {
            return Self { key: None };
        };
        if store::write(&beside(journal), &hex(&bytes)).is_err() {
            return Self { key: None };
        }
        Self { key: Some(key) }
    }

    /// The change as it is written down: nothing in it a credential can be read out of.
    ///
    /// Applied on the way to the file rather than where the change is made, so what a run
    /// holds in memory stays exact — an apply that stops part-way reverses its own writes
    /// from what it is holding — and only what outlives the run is sealed.
    ///
    /// It is also what closes a journal an older version left in clear. That file still
    /// reads exactly as it did; the first change recorded after the upgrade rewrites the
    /// whole of it, and every credential in it is sealed on the way back out.
    #[must_use]
    pub fn sealing(&self, change: &Change, random: &dyn Random) -> Change {
        both(change, &|value| {
            // A value that arrived still sealed is one this key did not open, and sealing
            // it again would bury it under a second key. Written back exactly as it was
            // read, restoring the key it *was* sealed under is still enough to read it.
            if is_sealed(value) {
                return value.to_owned();
            }
            self.seal(value, random)
                .unwrap_or_else(|| MARKER.to_owned())
        })
    }

    /// The change as it was made, where this key opens what was written down.
    ///
    /// A value that will not open is left as it was read, still marked, because the two
    /// reasons it would not — no key, or a record somebody edited — are both reasons to
    /// refuse to act on it rather than reasons to drop the entry. What changed is still
    /// worth reading even where what it changed to cannot be.
    #[must_use]
    pub fn opening(&self, change: &Change) -> Change {
        both(change, &|value| {
            self.open(value).unwrap_or_else(|| value.to_owned())
        })
    }

    /// One value sealed, or nothing where this machine would not supply what sealing
    /// needs — a key, or the nonce this value alone is sealed under.
    fn seal(&self, clear: &str, random: &dyn Random) -> Option<String> {
        let key = self.key.as_ref()?;
        let drawn = random.bytes(NONCE_BYTES)?;
        let nonce = XNonce::try_from(drawn.as_slice()).ok()?;
        let sealed = XChaCha20Poly1305::new(key)
            .encrypt(&nonce, clear.as_bytes())
            .ok()?;
        Some(format!("{MARKER}{}{}", hex(&drawn), hex(&sealed)))
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

/// The change with `through` applied to both halves of a credential setting, and every
/// other change exactly as it arrived.
///
/// One walk rather than two, because sealing and opening differ only in what they do to a
/// value — and a second copy of "which changes hold a credential" is a second answer to
/// that question waiting to disagree with the first.
///
/// Only [`Kind::Set`], which is a value in lemonfiber's own environment file. A field
/// changed inside a service is not sealed: the one thing that records those is the
/// download-client category, and a rule with nothing behind it is a rule nobody
/// maintains.
fn both(change: &Change, through: &dyn Fn(&str) -> String) -> Change {
    let Kind::Set {
        key,
        previous,
        current,
    } = &change.kind
    else {
        return change.clone();
    };
    if !is_secret(key) {
        return change.clone();
    }
    Change {
        kind: Kind::Set {
            key: key.clone(),
            previous: previous.as_deref().map(through),
            current: through(current),
        },
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

/// The bytes as hexadecimal, which is what a JSON string carries without escaping.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut written, byte| {
        // Writing into a `String` cannot fail — its `fmt::Write` is infallible — so
        // there is no error here to report and none to invent a branch for.
        let _ = write!(written, "{byte:02x}");
        written
    })
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
