//! What the core and the decline service hand each other.
//!
//! Three files, in the service's configuration directory:
//!
//! - [`File::Table`], the invitations it may act on, written by the core at each
//!   invitation and mounted read-only. A token is held only as its hash.
//! - [`File::Key`], the one Jellyfin API key minted for it, written by the core
//!   owner-only and mounted read-only.
//! - [`File::Refusals`], what it declined, written by the service and read back by
//!   the core, which reports each as the invitation's standing.

use serde::{Deserialize, Serialize};

use crate::shape::written;
pub use crate::TokenHash;
use crate::Unreadable;

/// The files in the decline service's configuration directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum File {
    /// The invitations the service may act on.
    Table,
    /// The Jellyfin API key minted for it alone.
    Key,
    /// What it declined.
    Refusals,
}

impl File {
    /// The file's name in the configuration directory.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Table => "invitations.json",
            Self::Key => "jellyfin.key",
            Self::Refusals => "refusals.json",
        }
    }
}

/// The shape of [`Table`] and [`Refusals`] this build reads and writes. A file in
/// another shape is refused rather than read as far as it happens to agree.
pub const FORMAT: u32 = 1;

/// One invitation the service may decline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Invitation {
    /// The hash of its decline token.
    pub token: TokenHash,
    /// The media server's identifier for the account made for it.
    pub account: String,
    /// The name the account was made under, which the page names.
    pub name: String,
    /// When it was issued, in seconds since the Unix epoch. A password change on the
    /// account after this is a claim.
    pub issued: u64,
    /// When it lapses, in seconds since the Unix epoch.
    pub lapses: u64,
}

impl Invitation {
    /// Whether it is still open at `now`, in seconds since the Unix epoch.
    #[must_use]
    pub fn open_at(&self, now: u64) -> bool {
        now < self.lapses
    }
}

/// The invitations the service may act on, as the core writes them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Table {
    /// The shape this file is written in; [`FORMAT`] for this build.
    pub format: u32,
    /// Every invitation, open or lapsed.
    pub invitations: Vec<Invitation>,
}

impl Table {
    /// A table holding `invitations`.
    #[must_use]
    pub fn of(invitations: Vec<Invitation>) -> Self {
        Self {
            format: FORMAT,
            invitations,
        }
    }

    /// The invitation `token` declines, if the table holds one.
    #[must_use]
    pub fn find(&self, token: &TokenHash) -> Option<&Invitation> {
        self.invitations.iter().find(|one| &one.token == token)
    }

    /// The table as it is written to [`File::Table`].
    #[must_use]
    pub fn written(&self) -> String {
        written(self)
    }

    /// The table [`File::Table`] holds.
    ///
    /// # Errors
    ///
    /// [`Unreadable`] where the text is not a table, or a table in another format.
    pub fn read(text: &str) -> Result<Self, Unreadable> {
        let table: Self = read(File::Table, text)?;
        formatted(File::Table, table.format)?;
        Ok(table)
    }
}

/// One invitation the service declined.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Refusal {
    /// The hash of the token it was declined with.
    pub token: TokenHash,
    /// The account it disabled.
    pub account: String,
    /// When, in seconds since the Unix epoch.
    pub at: u64,
}

/// What the service declined, as it records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Refusals {
    /// The shape this file is written in; [`FORMAT`] for this build.
    pub format: u32,
    /// Every refusal, oldest first.
    pub refusals: Vec<Refusal>,
}

impl Default for Refusals {
    fn default() -> Self {
        Self {
            format: FORMAT,
            refusals: Vec::new(),
        }
    }
}

impl Refusals {
    /// The refusal of the invitation `token` declines, if it was declined.
    #[must_use]
    pub fn of(&self, token: &TokenHash) -> Option<&Refusal> {
        self.refusals.iter().find(|one| &one.token == token)
    }

    /// These refusals and `refusal`, which an invitation already declined does not
    /// add to: a second refusal records nothing.
    #[must_use]
    pub fn with(mut self, refusal: Refusal) -> Self {
        if self.of(&refusal.token).is_none() {
            self.refusals.push(refusal);
        }
        self
    }

    /// The refusals as they are written to [`File::Refusals`].
    #[must_use]
    pub fn written(&self) -> String {
        written(self)
    }

    /// The refusals [`File::Refusals`] holds.
    ///
    /// # Errors
    ///
    /// [`Unreadable`] where the text is not a record of refusals, or one in another
    /// format.
    pub fn read(text: &str) -> Result<Self, Unreadable> {
        let refusals: Self = read(File::Refusals, text)?;
        formatted(File::Refusals, refusals.format)?;
        Ok(refusals)
    }
}

/// The Jellyfin API key minted for the decline service alone.
///
/// Its `Debug` withholds it, so a key carried in a value that is logged is not
/// logged with it.
#[derive(Clone, PartialEq, Eq)]
pub struct Key(String);

impl std::fmt::Debug for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Key(withheld)")
    }
}

impl Key {
    /// The key `text` holds, a line ending and surrounding space aside.
    ///
    /// # Errors
    ///
    /// [`Unreadable`] where it holds nothing, or more than one word.
    pub fn read(text: &str) -> Result<Self, Unreadable> {
        let key = text.trim();
        if key.is_empty() {
            return Err(crate::shape::unreadable(
                File::Key.name(),
                "it holds no key",
            ));
        }
        if key.split_whitespace().nth(1).is_some() {
            return Err(crate::shape::unreadable(
                File::Key.name(),
                "it holds more than one word",
            ));
        }
        Ok(Self(key.to_owned()))
    }

    /// The key as it is written to [`File::Key`].
    #[must_use]
    pub fn written(&self) -> String {
        format!("{}\n", self.0)
    }

    /// The key itself, for the one header it is sent in.
    #[must_use]
    pub fn reveal(&self) -> &str {
        &self.0
    }

    /// The key's SHA-256, in lowercase hexadecimal: what the service says it holds,
    /// and what the core compares against the key it wrote before revoking the old one.
    #[must_use]
    pub fn fingerprint(&self) -> String {
        TokenHash::of(&self.0).as_str().to_owned()
    }
}

/// What the service answers on its health route: the fingerprint of the key it holds,
/// or nothing where the core has not written one.
///
/// The core reads it after writing a new key, and revokes the old one only once the
/// service says it holds the new.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Health {
    /// [`Key::fingerprint`] of the key the service holds.
    pub key: Option<String>,
}

impl Health {
    /// The health of a service holding `key`, or none.
    #[must_use]
    pub fn holding(key: Option<&Key>) -> Self {
        Self {
            key: key.map(Key::fingerprint),
        }
    }

    /// Whether the service holds `key`.
    #[must_use]
    pub fn holds(&self, key: &Key) -> bool {
        self.key.as_deref() == Some(key.fingerprint().as_str())
    }
}

/// `text` read as the shape `file` should hold.
fn read<T: serde::de::DeserializeOwned>(file: File, text: &str) -> Result<T, Unreadable> {
    crate::shape::read(file.name(), text)
}

/// Whether `format` is the one this build reads.
fn formatted(file: File, format: u32) -> Result<(), Unreadable> {
    crate::shape::formatted(file.name(), format, FORMAT)
}

#[cfg(test)]
mod tests;
