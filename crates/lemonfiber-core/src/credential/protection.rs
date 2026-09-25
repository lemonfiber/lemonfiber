//! What keeping credentials in files does, and does not, protect against.
//!
//! Written as two lists because one list invites a reader to fill in the other, and
//! what they fill in is always more generous than the truth. A product that says
//! "your credentials are stored securely" has told the operator they may keep an
//! unencrypted backup on a shared drive; a product that says what the protection
//! actually is has told them why they may not.
//!
//! The first sentence is the one that matters and it is the one most likely to be
//! softened in review: the values are stored as text, not encrypted. There is no key
//! and no passphrase, because there is nowhere on an unattended machine to keep one
//! that the stack could not reach — and a stack that can reach the key is a stack
//! whose "encryption" is obfuscation with a longer word. Saying so is worth more than
//! the appearance of a guarantee nobody could keep.

use serde::Serialize;

/// What the storage is, in one sentence, before either list.
const PLAINLY: &str = "Credentials are stored as text in files owned by you. They are not \
encrypted: anything that can read the file can read the credential.";

/// What owner-only files genuinely stop.
const AGAINST: &[&str] = &[
    "another person with their own account on this machine reading them",
    "a service in the stack reading a credential meant for a different service, since \
     each is handed only what it authenticates with",
    "them appearing in a support bundle, a diagnosis, a log line or an error message, \
     which are filtered on the way out",
];

/// What they do not stop, said in the operator's own terms.
const NOT_AGAINST: &[&str] = &[
    "anything running as you, which includes any program you install and any malware \
     that reaches your account",
    "an unencrypted backup or a copy taken off this machine — a backup is exactly as \
     sensitive as the credentials in it",
    "an administrator of this machine, who can read any file on it",
    "anyone who reads the disk while it is unlocked, or an unencrypted disk taken out \
     of the machine",
];

/// The honest account of what the credential store protects against.
///
/// Two lists and a sentence, carried as data rather than printed here, so the API
/// serves the same words the terminal prints and neither can drift into a claim the
/// other does not make.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Protection {
    /// What the storage is, before any claim about it.
    pub summary: String,
    /// What it protects against.
    pub against: Vec<String>,
    /// What it does not protect against.
    pub not_against: Vec<String>,
}

impl Protection {
    /// The statement, as it stands.
    #[must_use]
    pub fn stated() -> Self {
        Self {
            summary: PLAINLY.to_owned(),
            against: AGAINST.iter().map(|said| (*said).to_owned()).collect(),
            not_against: NOT_AGAINST.iter().map(|said| (*said).to_owned()).collect(),
        }
    }
}

#[cfg(test)]
mod tests;
