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

use argon2::password_hash::{PasswordHash, PasswordHasher as _, PasswordVerifier as _};
use argon2::password_hash::{Salt, SaltString};
use argon2::Argon2;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::store::{self, Failure};
use crate::error::{Code, Diagnose, Problem, Remedy, Severity, State};
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

/// Raised when a password is too short to stand in front of this.
pub const TOO_SHORT: Code = Code::new("ADMIT-1");

/// Raised when this machine will not supply the salt a record is made with.
pub const NO_SALT: Code = Code::new("ADMIT-2");

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
/// Four ways to come back with nothing, and every one of them is the randomness
/// port answering with something no record may be made from: nothing at all, too
/// few bytes to encode as a salt, too many to write one down, or a salt shorter
/// than the function itself will use. None of them falls back to a weaker record,
/// because a narrow salt is invisible in the result — it looks exactly like a wide
/// one right up until two machines write down the same password the same way.
fn written(password: &str, random: &dyn Random) -> Option<String> {
    let bytes = random.bytes(SALT_BYTES)?;
    let encoded = SaltString::encode_b64(&bytes).ok()?;
    let salt = Salt::from_b64(encoded.as_str()).ok()?;
    let hashed = Argon2::default()
        .hash_password(password.as_bytes(), salt)
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
mod tests {
    use lemonfiber_fixtures::ports::Chance;
    use lemonfiber_fixtures::support::a_password;

    use super::{at, forget, keep, written, Credential, Weak, LEAST, SALT_BYTES};
    use crate::error::Diagnose as _;

    /// The randomness a real machine supplies: as many bytes as were asked for.
    fn a_machine() -> Chance {
        Chance::cycling()
    }

    /// Bytes a test chose, standing in for a source that answers with the wrong
    /// number of them.
    fn answering(count: usize) -> Chance {
        Chance::exactly(Some(vec![0x5a; count]))
    }

    /// A password long enough to be taken, built rather than written down.
    fn chosen() -> String {
        a_password()
    }

    /// A different one, so a verifier has something to refuse.
    fn another() -> String {
        chosen().to_uppercase()
    }

    /// The record a machine that answers makes of that password.
    ///
    /// Handed back as it came rather than opened here: a test that unwrapped it
    /// would carry a way out nothing takes, and the gate this workspace holds
    /// itself to counts that line like any other.
    fn a_record() -> Option<Credential> {
        Credential::set(&chosen(), &a_machine()).ok()
    }

    #[test]
    fn a_record_names_the_function_its_costs_and_its_version() {
        let written = a_record().map(|held| held.to_json()).unwrap_or_default();
        // The whole choice, pinned against the artefact rather than against the
        // constants that produced it: the variant, the format version, and all three
        // costs. A dependency bump that quietly lowered any of them is red here.
        assert!(
            written.contains("$argon2id$v=19$m=19456,t=2,p=1$"),
            "{written}"
        );
    }

    #[test]
    fn the_password_itself_is_nowhere_in_what_is_kept() {
        let held = a_record();
        let written = held.as_ref().map_or_else(chosen, Credential::to_json);
        assert!(!written.contains(&chosen()));
        assert!(written.starts_with("{\"verifier\":"), "{written}");
        assert_eq!(
            held.map(|held| format!("{held:?}")).unwrap_or_default(),
            "Credential(withheld)"
        );
    }

    #[test]
    fn a_record_proves_the_password_that_made_it_and_no_other() {
        let held = a_record();
        assert!(held.as_ref().is_some_and(|held| held.verifies(&chosen())));
        assert!(!held.as_ref().is_some_and(|held| held.verifies(&another())));
        assert!(!held.as_ref().is_some_and(|held| held.verifies("")));
    }

    #[test]
    fn two_records_of_one_password_are_written_down_differently() {
        // Different salts, so two operators who chose the same password are not
        // recognisable as having done so, and one cracked record is one record.
        let first = written(&chosen(), &answering(SALT_BYTES));
        let second = written(&chosen(), &a_machine());
        assert!(first.is_some() && second.is_some());
        assert_ne!(first, second);
    }

    #[test]
    fn a_password_shorter_than_the_floor_is_refused_and_told_the_floor() {
        let short: String = chosen().chars().take(LEAST - 1).collect();
        assert_eq!(
            Credential::set(&short, &a_machine()),
            Err(Weak::Short { least: LEAST })
        );
        let problem = Weak::Short { least: LEAST }.problem();
        assert!(problem
            .remedies
            .iter()
            .any(|remedy| remedy.action.contains(&LEAST.to_string())));
        assert!(Weak::Short { least: LEAST }
            .to_string()
            .contains(&LEAST.to_string()));
    }

    #[test]
    fn a_source_that_will_not_answer_leaves_no_record_rather_than_a_weak_one() {
        assert_eq!(
            Credential::set(&chosen(), &Chance::exactly(None)),
            Err(Weak::Unsalted)
        );
        let problem = Weak::Unsalted.problem();
        assert!(!problem.remedies.is_empty());
        assert!(Weak::Unsalted.to_string().contains("salt"));
    }

    #[test]
    fn a_salt_of_the_wrong_width_leaves_no_record_either() {
        // Three ways for a source to answer with something no record may be made
        // from, and each is refused where it is noticed: too few bytes to encode as
        // a salt at all, too many to be written into one, and a salt the function
        // itself will not use for being narrower than it requires.
        for count in [2, 4, 64] {
            assert_eq!(written(&chosen(), &answering(count)), None, "{count} bytes");
        }
        assert!(written(&chosen(), &answering(SALT_BYTES)).is_some());
    }

    #[test]
    fn a_record_survives_being_written_down_and_read_back() {
        let held = a_record();
        let written = held.as_ref().map(Credential::to_json).unwrap_or_default();
        let read = Credential::parse(&written);
        assert_eq!(read.clone(), held.clone());
        assert!(read.is_some_and(|read| read.verifies(&chosen())));
        assert_eq!(held.clone(), held);
    }

    #[test]
    fn a_record_that_cannot_be_read_proves_nothing_rather_than_everything() {
        assert_eq!(Credential::parse("not a record"), None);
        let damaged = Credential::parse(r#"{"verifier":"nonsense"}"#);
        assert!(damaged.is_some_and(|damaged| !damaged.verifies(&chosen())));
    }

    #[test]
    fn a_credential_is_kept_read_back_and_forgotten() {
        let dir = std::env::temp_dir().join(format!("lemonfiber-admission-{}", std::process::id()));
        let path = dir.join("admission.json");
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(at(&path), None);
        let held = a_record();
        assert!(held.as_ref().is_some_and(|held| keep(&path, held).is_ok()));
        assert!(at(&path).is_some_and(|held| held.verifies(&chosen())));
        assert!(forget(&path).is_ok());
        assert_eq!(at(&path), None);
        // Forgetting what is already forgotten is what was asked for.
        assert!(forget(&path).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_credential_that_cannot_be_written_or_removed_says_so() {
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-admission-x-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(std::fs::create_dir_all(&dir).is_ok());
        // A directory where the file should be: it can neither be written over nor
        // removed as a file, which is the pair of failures worth reporting.
        assert!(a_record()
            .as_ref()
            .is_some_and(|held| keep(&dir, held).is_err()));
        assert!(forget(&dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
