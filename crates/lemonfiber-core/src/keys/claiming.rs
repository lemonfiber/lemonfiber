//! Keys minted as a claim, and collected once by the program they are for.
//!
//! A program on a device with a camera takes its key from a code on a screen. Putting
//! the secret in that code would make a photograph of the screen a working key, so the
//! code carries a claim instead: the stack's address, its certificate and a value good
//! for collecting the key once, within [`MINUTES_TO_COLLECT`] minutes.
//!
//! **The secret does not exist until the key is collected.** It is minted in the reply
//! that collects it, so nothing kept between the mint and the collection could hand it
//! over, and a key nobody collects never had one. The claim itself is kept only as a
//! digest, as a secret is.

use serde::{Deserialize, Serialize};

use super::{digest, minted_as, shaped_as, Purpose};
use crate::ports::random::Random;

/// What every claim begins with, so a value shaped like one is told from a secret.
pub const CLAIM_PREFIX: &str = "lfc_";

/// How long a claim collects its key for, from the moment it was minted.
pub const MINUTES_TO_COLLECT: i64 = 10;

/// Where a program opens a key's claim.
pub(crate) const KEY_LINK: &str = "lemonfiber://key";

/// A claim, as it goes out once in a key link.
#[derive(Clone, PartialEq, Eq)]
pub struct Claim(String);

/// What is said instead of a claim.
impl std::fmt::Debug for Claim {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str("Claim(withheld)")
    }
}

impl Claim {
    /// Mint one, or nothing where the operating system will not supply the bytes.
    pub fn mint(random: &dyn Random) -> Option<Self> {
        minted_as(random, CLAIM_PREFIX).map(Self)
    }

    /// The claim as it is written into a link.
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

/// Whether `offered` is shaped like a claim: the prefix and the width after it.
#[must_use]
pub fn claim_shaped(offered: &str) -> bool {
    shaped_as(offered, CLAIM_PREFIX)
}

/// The claim a key waits to be collected with, as it is kept.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pending {
    /// The SHA-256 of the claim, in lower-case hex.
    pub(super) digest: String,
    /// When it stops collecting anything, written as every other instant is.
    pub lapses: String,
}

/// What is said instead of a pending claim: when it lapses, and not its digest.
impl std::fmt::Debug for Pending {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("Pending")
            .field("lapses", &self.lapses)
            .finish_non_exhaustive()
    }
}

impl Pending {
    /// Whether it has stopped collecting anything by `now`.
    ///
    /// A lapse that cannot be read has lapsed: a claim whose window nobody can tell is
    /// not one to honour.
    #[must_use]
    pub fn lapsed(&self, now: std::time::SystemTime) -> bool {
        crate::instant::read(&self.lapses).is_none_or(|lapses| now >= lapses)
    }
}

/// A key minted as a claim: the link that collects it, and no secret.
#[derive(Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "KeyClaim")]
pub struct Claimed {
    /// The name it was minted under.
    pub name: String,
    /// What it admits: `read`, `act` or `member:<name>`.
    pub scope: String,
    /// What the minter said it is for.
    pub purpose: Purpose,
    /// The key link: `lemonfiber://key` with the address, the certificate fingerprint,
    /// the stack, when the claim lapses and the claim. Shown once, and drawn as a code.
    pub link: String,
    /// When the claim stops collecting the key, written as every other instant is.
    pub lapses: String,
}

/// What is said instead of a key claim: everything but the link, which holds the claim.
impl std::fmt::Debug for Claimed {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("Claimed")
            .field("name", &self.name)
            .field("scope", &self.scope)
            .field("purpose", &self.purpose)
            .field("link", &"withheld")
            .field("lapses", &self.lapses)
            .finish()
    }
}

#[cfg(test)]
mod tests;
