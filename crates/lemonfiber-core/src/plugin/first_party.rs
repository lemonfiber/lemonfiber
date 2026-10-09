//! Which installed plugins are first-party: the ones this build embeds from the signed
//! default bundle, by id and the digest of their manifest.
//!
//! Read off the build rather than off the record, so a plugin the build no longer names
//! is third-party at once, however it was installed.

use super::Installed;

/// One first-party plugin, as the signed default bundle names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FirstParty {
    /// Its id.
    pub plugin: &'static str,
    /// The SHA-256 of its manifest, in lower-case hexadecimal.
    pub manifest: &'static str,
}

/// Every first-party plugin this build embeds.
pub const EMBEDDED: &[FirstParty] = &[];

/// Whether `installed` is one of `set`, by id and manifest alike.
#[must_use]
pub fn holds(set: &[FirstParty], installed: &Installed) -> bool {
    set.iter()
        .any(|one| one.plugin == installed.plugin && one.manifest == installed.manifest)
}

#[cfg(test)]
mod tests;
