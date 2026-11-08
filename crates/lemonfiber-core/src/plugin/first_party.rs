//! Which installed plugins are first-party: the ones this build embeds from the signed
//! default bundle, by id and the digests of their manifest and their tree.
//!
//! Read off the build rather than off the record, so a plugin the build no longer names
//! is third-party at once, however it was installed.

use super::Installed;

pub use lemonfiber_bundled::FirstParty;

/// Every first-party plugin this build embeds, as the bundle pins them.
pub const EMBEDDED: &[FirstParty] = lemonfiber_bundled::FIRST_PARTY;

/// Whether `installed` is one of `set`, by id, manifest and tree alike.
#[must_use]
pub fn holds(set: &[FirstParty], installed: &Installed) -> bool {
    named(set, &installed.plugin, &installed.manifest, &installed.tree)
}

/// Whether `set` names the plugin `plugin` whose manifest has the digest `manifest` and
/// whose tree has the digest `tree`.
#[must_use]
pub fn named(set: &[FirstParty], plugin: &str, manifest: &str, tree: &str) -> bool {
    set.iter().any(|one| {
        one.plugin == plugin && one.manifest == manifest && !one.tree.is_empty() && one.tree == tree
    })
}

#[cfg(test)]
mod tests;
