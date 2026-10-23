//! The keys another program holds to reach the web surface, kept as digests.
//!
//! The per-run token and a password session suit a person in front of a screen. A
//! program that runs for months beside the stack needs a credential that outlives a
//! restart and can do less than the operator, so the operator mints one for it under a
//! name, with one scope, and hands over the secret once.
//!
//! **The secret is never kept.** What is written down is its SHA-256 digest. A secret
//! is thirty-two bytes the operating system chose, so there is nothing to guess at and
//! nothing a slow hash would buy, which is why this is not the password's Argon2id: a
//! digest checked on every request has to be cheap, and the width is what makes it
//! hopeless to reverse.
//!
//! **A secret is recognisable as one.** Each begins with [`PREFIX`], so the web surface
//! can tell a value shaped like a key from a token or a session without looking it up,
//! and count only the ones that match no key against the limit wrong passwords meet.
//!
//! **A name identifies one key, for good.** A revoked key keeps its name in the listing,
//! with when it was revoked, so a name read in an old alert or journal entry still
//! names the one key it named then.

pub mod claiming;
mod held;
pub mod listing;
pub mod run;
pub mod scope;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::store;
use crate::error::codes::key::{
    BAD_NAME, CLAIM_NOT_OPEN, MEMBERS_MAY_NOT_MINT, NAME_TAKEN, NOT_A_PURPOSE, NOT_A_SCOPE,
    NOT_FOR_YOURSELF, NO_SUCH_KEY, NO_SUCH_MEMBER, UNLINKABLE,
};
use crate::error::Code;
use crate::ports::random::Random;
use crate::secret::render;

pub use claiming::{Claim, Claimed, Pending, CLAIM_PREFIX};
pub(crate) use held::Held;
pub use listing::{Listed, Listing, Minted, State};
pub use scope::{Minter, Purpose, Scope, Wanted};

/// What every secret begins with.
pub const PREFIX: &str = "lfk_";

/// Every refusal minting, collecting or revoking a key answers with, which a client names
/// by its code.
pub const REFUSALS: [Code; 10] = [
    BAD_NAME,
    NOT_A_SCOPE,
    NOT_A_PURPOSE,
    NAME_TAKEN,
    NO_SUCH_MEMBER,
    NOT_FOR_YOURSELF,
    NO_SUCH_KEY,
    MEMBERS_MAY_NOT_MINT,
    CLAIM_NOT_OPEN,
    UNLINKABLE,
];

pub use crate::config::paths::KEYS as FILE;

pub use crate::config::paths::KEYS_USED as USED_FILE;

/// Bytes of secret. Wide enough that guessing is not a strategy.
const WIDTH: usize = 32;

/// The most characters a key's name may have.
pub const LONGEST_NAME: usize = 64;

/// A key's secret, as it is handed over once.
///
/// It has no `Debug` that prints it and no way back from its digest, and nothing here
/// writes it anywhere: the one reply that carries it is the only place it appears.
#[derive(Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct Secret(String);

/// What is said instead of a secret.
impl std::fmt::Debug for Secret {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str("Secret(withheld)")
    }
}

impl Secret {
    /// Mint one, or nothing where the operating system will not supply the bytes.
    ///
    /// A source that answers with fewer bytes than it was asked for is one that would
    /// not say: a narrower secret looks exactly like a wide one until somebody guesses
    /// it.
    pub fn mint(random: &dyn Random) -> Option<Self> {
        minted_as(random, PREFIX).map(Self)
    }

    /// The secret as a client sends it back.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// What is kept in its place.
    #[must_use]
    pub fn digest(&self) -> String {
        digest(&self.0)
    }
}

/// Bytes in a secret or a claim, rendered as `prefix` and lower-case hex, or nothing
/// where the operating system will not supply every byte asked for.
fn minted_as(random: &dyn Random, prefix: &str) -> Option<String> {
    let bytes = random.bytes(WIDTH)?;
    (bytes.len() == WIDTH).then(|| format!("{prefix}{}", render(&bytes)))
}

/// Whether `offered` is `prefix` followed by a minted value's width of lower-case hex.
fn shaped_as(offered: &str, prefix: &str) -> bool {
    offered.strip_prefix(prefix).is_some_and(|rest| {
        rest.len() == WIDTH * 2
            && rest
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    })
}

/// The SHA-256 of `offered`, in lower-case hex.
#[must_use]
pub fn digest(offered: &str) -> String {
    render(ring::digest::digest(&ring::digest::SHA256, offered.as_bytes()).as_ref())
}

/// Whether `offered` is shaped like a key's secret: the prefix and the width after it,
/// in lower-case hex.
#[must_use]
pub fn shaped(offered: &str) -> bool {
    shaped_as(offered, PREFIX)
}

/// Whether two digests are the same, looked at over every byte whatever is found.
pub(crate) fn same(kept: &str, offered: &str) -> bool {
    let (kept, offered) = (kept.as_bytes(), offered.as_bytes());
    kept.len() == offered.len()
        && kept
            .iter()
            .zip(offered)
            .fold(0u8, |seen, (a, b)| seen | (a ^ b))
            == 0
}

/// Whether a word may name a key.
///
/// Lower-case letters, digits and the three marks a name is usually joined with,
/// beginning with a letter or a digit. A name travels in a path on the web surface, in
/// an alert and in the journal, so it carries nothing any of them would have to escape.
#[must_use]
pub fn names_a_key(name: &str) -> bool {
    let mut letters = name.bytes();
    letters
        .next()
        .is_some_and(|first| first.is_ascii_lowercase() || first.is_ascii_digit())
        && name.len() <= LONGEST_NAME
        && letters.all(|letter| {
            letter.is_ascii_lowercase()
                || letter.is_ascii_digit()
                || matches!(letter, b'-' | b'_' | b'.')
        })
}

/// One key as it is kept.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    /// What the operator called it.
    pub name: String,
    /// What it admits.
    pub scope: Scope,
    /// What the minter said it is for.
    pub purpose: Purpose,
    /// The SHA-256 of its secret, in lower-case hex, or nothing for a key minted as a
    /// claim and not yet collected, which has no secret yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    digest: Option<String>,
    /// The claim it waits to be collected with, where it was minted as one and nobody
    /// has collected it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending: Option<Pending>,
    /// When it was minted, written as every other instant this product writes.
    pub minted: String,
    /// Who minted it.
    pub by: Minter,
    /// When it was revoked, where it has been.
    #[serde(default)]
    pub revoked: Option<String>,
    /// The device key whose signature proved its mint, where one did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proved_by: Option<String>,
}

/// What is said instead of a record: everything but the digest.
///
/// The digest proves nothing on its own, and is still kept out of anything printed: a
/// record pasted into a forum should say which key it was and nothing a reader could
/// compare a guess against.
impl std::fmt::Debug for Record {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("Record")
            .field("name", &self.name)
            .field("scope", &self.scope)
            .field("purpose", &self.purpose)
            .field("minted", &self.minted)
            .field("by", &self.by)
            .field("revoked", &self.revoked)
            .field("proved_by", &self.proved_by)
            .field(
                "pending",
                &self.pending.as_ref().map(|pending| &pending.lapses),
            )
            .finish_non_exhaustive()
    }
}

impl Record {
    /// A key minted now for `secret`.
    #[must_use]
    pub fn minted(
        name: &str,
        scope: Scope,
        purpose: Purpose,
        secret: &Secret,
        minted: String,
        by: Minter,
    ) -> Self {
        Self {
            name: name.to_owned(),
            scope,
            purpose,
            digest: Some(secret.digest()),
            pending: None,
            minted,
            by,
            revoked: None,
            proved_by: None,
        }
    }

    /// A key minted now as `claim`, waiting until `lapses` to be collected, with no
    /// secret yet.
    #[must_use]
    pub fn claimed(
        name: &str,
        scope: Scope,
        purpose: Purpose,
        claim: &Claim,
        lapses: String,
        minted: String,
        by: Minter,
    ) -> Self {
        Self {
            name: name.to_owned(),
            scope,
            purpose,
            digest: None,
            pending: Some(Pending {
                digest: claim.digest(),
                lapses,
            }),
            minted,
            by,
            revoked: None,
            proved_by: None,
        }
    }

    /// The same key, proved by the device key `device` where one proved it.
    #[must_use]
    pub fn proved_by(self, device: Option<&str>) -> Self {
        Self {
            proved_by: device.map(str::to_owned),
            ..self
        }
    }

    /// This key collected with `secret`: the secret's digest kept, the claim gone.
    pub fn collected(&mut self, secret: &Secret) {
        self.digest = Some(secret.digest());
        self.pending = None;
    }

    /// Whether this key waits to be collected.
    #[must_use]
    pub const fn is_waiting(&self) -> bool {
        self.pending.is_some() && self.revoked.is_none()
    }

    /// Whether this key has been revoked.
    #[must_use]
    pub const fn is_revoked(&self) -> bool {
        self.revoked.is_some()
    }
}

/// Every key this machine has minted, revoked ones included.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kept {
    /// In the order they were minted.
    #[serde(default)]
    pub keys: Vec<Record>,
}

/// Why the keys could not be read or written.
#[derive(Debug)]
pub enum Unkept {
    /// The file is there and does not read as keys. Nothing is written over it.
    Unreadable(PathBuf),
    /// It could not be written.
    NotWritten(store::Failure),
}

impl Kept {
    /// The keys kept at `path`: none where there is no file, and why not where there is
    /// one that does not read.
    ///
    /// # Errors
    ///
    /// [`Unkept::Unreadable`] where a file is there and is not keys.
    pub fn at(path: &Path) -> Result<Self, Unkept> {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                serde_json::from_str(&text).map_err(|_| Unkept::Unreadable(path.to_path_buf()))
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(_) => Err(Unkept::Unreadable(path.to_path_buf())),
        }
    }

    /// Write them, owner-only.
    ///
    /// # Errors
    ///
    /// [`Unkept::NotWritten`] where the file could not be written.
    pub fn keep(&self, path: &Path) -> Result<(), Unkept> {
        let text = serde_json::to_string_pretty(self).unwrap_or_default();
        store::write(path, &text).map_err(Unkept::NotWritten)
    }

    /// The key holding `name`, revoked or not.
    #[must_use]
    pub fn named(&self, name: &str) -> Option<&Record> {
        self.keys.iter().find(|record| record.name == name)
    }

    /// Every active key the device key `device` proved the mint of.
    #[must_use]
    pub fn proved_with(&self, device: &str) -> Vec<&Record> {
        self.keys
            .iter()
            .filter(|record| !record.is_revoked() && record.proved_by.as_deref() == Some(device))
            .collect()
    }

    /// The key whose secret `offered` is, where it is one.
    ///
    /// Every digest kept is compared over every byte, and the walk does not stop at a
    /// match, so how long this takes says nothing about which key matched or how much
    /// of a guess was right.
    #[must_use]
    pub fn holding(&self, offered: &str) -> Option<&Record> {
        let offered = digest(offered);
        self.keys.iter().fold(None, |found, record| {
            let matches = same(record.digest.as_deref().unwrap_or_default(), &offered);
            if matches && found.is_none() {
                Some(record)
            } else {
                found
            }
        })
    }

    /// The index of the key waiting to be collected with `offered`, where one is.
    ///
    /// Compared over every kept claim's every byte, as [`Kept::holding`] compares
    /// secrets, so the time it takes says nothing about which claim matched.
    #[must_use]
    pub fn awaiting(&self, offered: &str) -> Option<usize> {
        let offered = digest(offered);
        self.keys
            .iter()
            .enumerate()
            .fold(None, |found, (at, record)| {
                let kept = record
                    .pending
                    .as_ref()
                    .map(|pending| pending.digest.as_str())
                    .unwrap_or_default();
                if same(kept, &offered) && record.revoked.is_none() && found.is_none() {
                    Some(at)
                } else {
                    found
                }
            })
    }
}

/// Where each key was last used, by name, as the serving surface writes it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Used {
    /// Name to the instant it was last admitted, written as every other instant is.
    #[serde(default)]
    pub at: std::collections::BTreeMap<String, String>,
}

impl Used {
    /// What is kept at `path`, or nothing recorded where it cannot be read.
    ///
    /// A last use that cannot be read is a listing that says *never*, which is wrong in
    /// the safe direction: an operator deciding what to revoke is told a key has not been
    /// used rather than that it has.
    #[must_use]
    pub fn at(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    /// Write it, owner-only, best effort.
    pub fn keep(&self, path: &Path) {
        let text = serde_json::to_string_pretty(self).unwrap_or_default();
        let _ = store::write(path, &text);
    }
}

#[cfg(test)]
mod tests;
