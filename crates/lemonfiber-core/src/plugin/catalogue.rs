//! The catalogue's index, whether its signature holds, and what a name resolves to.
//!
//! A catalogue release is a tag whose assets are an index — each plugin's id, the
//! origin it is fetched from, the commit that was reviewed and the digest of its
//! manifest there — and a signature over exactly the index's bytes. A name is resolved
//! through an index whose signature verified against the key this build carries, and
//! through nothing else: an index with no signature, one whose signature does not
//! verify, and one read by a build that carries no key are all refused the same way,
//! because an unverifiable claim to have reviewed something is worse than none.
//!
//! **Nothing here reaches the network.** What the release served arrives as text
//! already fetched, so every way of it not holding can be put in front of this
//! without one.

use serde::Deserialize;

use super::provenance::{decoded, Key};

/// Where the newest catalogue release's index is fetched from.
///
/// The address that answers with the newest release's asset, so a catalogue that
/// tags a new release is read at its newest without this build naming a tag.
pub const INDEX: &str =
    "https://github.com/lemonfiber/lemonfiber-plugins/releases/latest/download/index.json";

/// Where the signature over that index is fetched from.
pub const SIGNATURE: &str =
    "https://github.com/lemonfiber/lemonfiber-plugins/releases/latest/download/index.json.sig";

/// The public half of the key catalogue releases are signed with, as PEM.
///
/// Empty in a build made before the key was published. Such a build verifies nothing,
/// so every install by name from it is refused as unverifiable rather than accepted.
const KEY: &str = "";

/// What the key is called where an install records what signed it.
const KEY_NAMED: &str = "the lemonfiber-plugins catalogue key";

/// The shape of index this build reads.
const SCHEMA: u32 = 1;

/// How a manifest's digest names the algorithm it was taken with.
const DIGEST: &str = "sha256:";

/// The key this build carries, or nothing where it carries none.
#[must_use]
pub fn carried() -> Option<Key> {
    Key::from_pem(KEY_NAMED, KEY).ok()
}

/// A catalogue release's index, as it was signed.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Index {
    /// The shape it is written in.
    schema: u32,
    /// Which release of the catalogue it is, raised by each release above the last.
    serial: u64,
    /// One entry per registered plugin.
    plugins: Vec<Entry>,
}

/// One plugin, as the catalogue registered and reviewed it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Entry {
    /// The plugin's id, which is the name it is installed by.
    pub id: String,
    /// The git repository it is fetched from.
    pub origin: String,
    /// The one commit of it somebody reviewed.
    pub revision: String,
    /// The digest of its manifest at that commit.
    pub manifest: String,
}

impl Index {
    /// Which release of the catalogue it is.
    #[must_use]
    pub const fn serial(&self) -> u64 {
        self.serial
    }

    /// The entry for one name, where the index holds one.
    #[must_use]
    pub fn entry(&self, id: &str) -> Option<&Entry> {
        self.plugins.iter().find(|one| one.id == id)
    }
}

impl Entry {
    /// Whether a manifest's bytes are the ones this entry's digest names.
    #[must_use]
    pub fn holds(&self, manifest: &[u8]) -> bool {
        self.reviewed(&crate::secret::render(
            ring::digest::digest(&ring::digest::SHA256, manifest).as_ref(),
        ))
    }

    /// Whether a manifest whose SHA-256 is `digest`, in lower-case hexadecimal, is the
    /// one this entry's digest names.
    #[must_use]
    pub fn reviewed(&self, digest: &str) -> bool {
        self.manifest == format!("{DIGEST}{digest}")
    }

    /// Whether its revision is one whole commit, which is the only thing an index may
    /// pin a plugin to.
    #[must_use]
    pub fn pins_a_commit(&self) -> bool {
        self.revision.len() == 40 && self.revision.bytes().all(|byte| byte.is_ascii_hexdigit())
    }
}

/// Why an index was not read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refused {
    /// Nothing ties it to what the catalogue signed.
    #[error("{0}")]
    Unverified(String),
    /// It verified, and this build cannot read what it says.
    #[error("{0}")]
    Unreadable(String),
}

/// The index, where its signature verifies against `key`.
///
/// # Errors
///
/// [`Refused::Unverified`] where there is no key to check against, where the
/// signature is not one, and where it does not verify over exactly these bytes.
/// [`Refused::Unreadable`] where it verifies and is not an index this build reads.
pub fn verified(index: &str, signature: &str, key: Option<&Key>) -> Result<Index, Refused> {
    let Some(key) = key else {
        return Err(Refused::Unverified(
            "this build carries no key to check the catalogue's signature against".to_owned(),
        ));
    };
    let signature = decoded(signature.trim()).ok_or_else(|| {
        Refused::Unverified("what was served as its signature is not base64".to_owned())
    })?;
    if !key.made(&signature, index.as_bytes()) {
        return Err(Refused::Unverified(format!(
            "its signature does not verify against {}",
            signer(key)
        )));
    }
    let read: Index = serde_json::from_str(index).map_err(|why| {
        Refused::Unreadable(format!("it is not an index this build reads: {why}"))
    })?;
    if read.schema != SCHEMA {
        return Err(Refused::Unreadable(format!(
            "it is written in shape {} and this build reads shape {SCHEMA}",
            read.schema
        )));
    }
    Ok(read)
}

/// What an install records as having signed a plugin: the key's name and its
/// fingerprint.
#[must_use]
pub fn signer(key: &Key) -> String {
    format!("{} ({DIGEST}{})", key.named, key.fingerprint())
}

#[cfg(test)]
pub(crate) mod signing;
#[cfg(test)]
mod tests;
