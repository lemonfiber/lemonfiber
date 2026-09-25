//! Who this surface lets in, and what it keeps in order to decide.
//!
//! Everything here is about the operator's own password — the one credential in
//! this product lemonfiber *verifies* rather than reads. Every other secret the
//! stack holds is a service's own, fetched so lemonfiber can present it; this one
//! arrives from a person and is never given back to anybody, which is a different
//! problem and is why it is kept apart from them.
//!
//! It exists because the web surface can start, stop and reconfigure the whole
//! stack, and because a household reaches lemonfiber from a phone rather than from
//! the machine it runs on. A secret printed on the terminal that started the
//! process answers "whoever is at this machine", which is the population loopback
//! already answers for. A password answers a different question, and it is the one
//! that has to be answered before anything beyond loopback is offered.
//!
//! The password the operator sets, as it is kept and as it is checked.
//!
//! Kept as a verifier and never as the password: what is on disk proves an answer
//! right without holding the answer, so a file somebody walks off with is a file
//! they still have to guess against. There is no key that would turn it back —
//! encrypting it at rest would only move the question to where the key is kept,
//! and a key that can decrypt is a key that can leak.
//!
//! **Argon2id**, at the parameters its own crate defaults to, which are the ones
//! OWASP recommends: 19 MiB of memory, two passes, one lane. Memory is the point.
//! A hash that costs only arithmetic is a hash an attacker runs on a graphics card
//! by the billion; one that costs 19 MiB per guess is bounded by memory bandwidth,
//! which is the resource that does not get cheaper by the rack. The `id` variant is
//! taken over either half: the first pass is data-independent, so a process sharing
//! the machine learns nothing from the memory access pattern, and every pass after
//! it is data-dependent, which is what closes the shortcut a purely data-independent
//! function leaves open.
//!
//! The parameters travel in the record itself — the stored string names the
//! algorithm, the version, the costs and the salt — so raising them later is a
//! change to what is written next rather than a migration of what is already
//! written.
//!
//! The salt arrives through the randomness port every other secret does, so this is
//! exercised against bytes a test chose rather than against whatever the machine
//! happened to produce, and the operating system's CSPRNG stays in the one adapter.

use std::path::Path;

use argon2::{Argon2, PasswordHash, PasswordHasher as _, PasswordVerifier as _};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::store::{self, Failure};
use crate::error::codes::admit::{NO_SALT, TOO_SHORT};
use crate::error::{Diagnose, Problem, Remedy, Severity, State};
use crate::ports::random::Random;
use crate::PRODUCT;

/// How many bytes of salt a record is made with.
///
/// Sixteen is what the format calls the recommended length, and it is far past the
/// point where two operators could be expected to share one.
const SALT_BYTES: usize = 16;

/// The fewest characters a password may have.
///
/// Counted in characters rather than bytes, so a passphrase written in a language
/// that spends more than one byte a letter is not penalised for it. Twelve because
/// this stands in front of everything the stack can do and the thing on the other
/// side of it is a machine that never gets bored, not a person who gives up.
pub const LEAST: usize = 12;

/// The operator's password, as it is kept: something that proves an answer right
/// and holds no answer.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credential {
    /// The stored verifier, in the format that carries its own parameters.
    verifier: String,
}

/// What is said instead of a credential.
///
/// Written out rather than derived. A derived one would print the verifier into
/// whatever printed the struct — a log line, a panic message, an error somebody
/// pasted into a forum — and the whole point of the thing is that it never goes
/// anywhere.
impl std::fmt::Debug for Credential {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str("Credential(withheld)")
    }
}

/// Why a password was not taken.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Weak {
    /// Shorter than the fewest characters that make guessing hopeless.
    #[error("a password of at least {least} characters is needed")]
    Short {
        /// The fewest characters that would have been accepted.
        least: usize,
    },
    /// This machine would not supply the randomness a record is salted with.
    #[error("no salt could be taken from this machine")]
    Unsalted,
}

impl Diagnose for Weak {
    fn problem(&self) -> Problem {
        match self {
            Self::Short { least } => Problem::new(
                TOO_SHORT,
                Severity::Error,
                "That password is too short to be the one",
                "This is the only thing standing in front of a surface that can start, stop \
                 and reconfigure everything, and what is on the other side of it is a program \
                 that guesses without getting bored.",
                Remedy::new(format!("Use at least {least} characters"))
                    .with_detail("Several unrelated words are easier to keep and harder to guess than one word with substitutions in it."),
            )
            .in_state(State::Guided),
            Self::Unsalted => Problem::new(
                NO_SALT,
                Severity::Error,
                format!("{PRODUCT} could not record that password"),
                "Every stored password is mixed with unpredictable bytes so that two of them \
                 are never written down the same way, and this machine would not supply any.",
                Remedy::new(
                    "Try again, and if it happens twice the operating system's own random \
                     source is at fault",
                ),
            )
            .in_state(State::Guided),
        }
    }
}

impl Credential {
    /// Take a password and write down what proves it, or say why not.
    ///
    /// # Errors
    ///
    /// Returns [`Weak::Short`] where the password is shorter than [`LEAST`], and
    /// [`Weak::Unsalted`] where this machine would not supply a salt to record it
    /// with.
    pub fn set(password: &str, random: &dyn Random) -> Result<Self, Weak> {
        if password.chars().count() < LEAST {
            return Err(Weak::Short { least: LEAST });
        }
        let Some(verifier) = written(password, random) else {
            return Err(Weak::Unsalted);
        };
        Ok(Self { verifier })
    }

    /// Whether this is the password that was set.
    ///
    /// A record that cannot be read proves nothing, which is the safe direction: a
    /// damaged file refuses everybody rather than admitting everybody.
    #[must_use]
    pub fn verifies(&self, offered: &str) -> bool {
        PasswordHash::new(&self.verifier).is_ok_and(|stored| {
            Argon2::default()
                .verify_password(offered.as_bytes(), &stored)
                .is_ok()
        })
    }

    /// What was kept, where it can be read back.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        serde_json::from_str(text).ok()
    }

    /// As it is stored.
    ///
    /// One string in one field, so there is no way for this to fail and no failure
    /// worth inventing for it. What an impossible one would write is nothing, which
    /// reads back as no credential — the same direction every other unreadable
    /// record here falls in.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// The record one password is written down as, or nothing where it could not be.
///
/// Three ways to come back with nothing, and every one of them is the randomness
/// port answering with something no record may be made from: nothing at all, fewer
/// bytes than the function will salt with, or more than a record holds. None of
/// them falls back to a weaker record, because a narrow salt is invisible in the
/// result — it looks exactly like a wide one right up until two machines write down
/// the same password the same way.
fn written(password: &str, random: &dyn Random) -> Option<String> {
    let bytes = random.bytes(SALT_BYTES)?;
    let hashed = Argon2::default()
        .hash_password_with_salt(password.as_bytes(), &bytes)
        .ok()?;
    Some(hashed.to_string())
}

/// The credential this machine holds, or nothing where it holds none.
///
/// Absent, unreadable and unreadable-as-a-credential are one answer, and
/// deliberately: every one of them means nothing here can prove who is knocking,
/// and the binding policy reads that as no authentication rather than as a fault to
/// be worked around. Getting it wrong in this direction costs an operator a
/// password to set again; getting it wrong in the other would leave a surface on the
/// network with nothing in front of it.
#[must_use]
pub fn at(path: &Path) -> Option<Credential> {
    std::fs::read_to_string(path)
        .ok()
        .as_deref()
        .and_then(Credential::parse)
}

/// Write it down, owner-only, where the rest of this machine's configuration lives.
///
/// # Errors
///
/// Returns [`Failure::NotWritten`] where the file could not be written.
pub fn keep(path: &Path, credential: &Credential) -> Result<(), Failure> {
    store::write(path, &credential.to_json())
}

/// Remove it, so nothing here can prove who is knocking.
///
/// A file that is not there is what was asked for rather than a failure: the
/// question is whether a credential is held afterwards, and it is not either way.
///
/// # Errors
///
/// Returns [`Failure::NotWritten`] where the file is there and could not be removed.
pub fn forget(path: &Path) -> Result<(), Failure> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(Failure::NotWritten {
            path: path.to_path_buf(),
            reason: err.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests;
