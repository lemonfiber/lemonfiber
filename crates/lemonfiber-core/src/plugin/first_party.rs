//! Which installed plugins are first-party: the ones this build embeds from the signed
//! default bundle, by id and the digest of their manifest.
//!
//! Read off the build rather than off the record, so a plugin the build no longer names
//! is third-party at once, however it was installed.

use super::Installed;

pub use lemonfiber_bundled::FirstParty;

/// Every first-party plugin this build embeds, as the bundle pins them.
pub const EMBEDDED: &[FirstParty] = lemonfiber_bundled::FIRST_PARTY;

/// Whether `installed` is one of `set`, by id and manifest alike.
#[must_use]
pub fn holds(set: &[FirstParty], installed: &Installed) -> bool {
    set.iter()
        .any(|one| one.plugin == installed.plugin && one.manifest == installed.manifest)
}

#[cfg(test)]
mod tests;
