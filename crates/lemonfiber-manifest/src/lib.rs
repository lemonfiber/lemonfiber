//! Parsing of `stack.toml`, the contract between `lemonfiber` and `media-stack`.
//! See spec `20-architecture/contracts/stack-manifest.md`.
//!
//! This crate is deliberately inert: it reads a manifest into types and refuses
//! one it cannot read. Deciding what to *do* with a manifest belongs to
//! `lemonfiber-core`, which is what lets both be tested with no Docker, no
//! terminal and no configuration.

mod error;
mod schema;

pub use error::Error;
pub use schema::{
    Api, ApiKind, Bind, Criticality, Form, Health, HealthKind, KeySource, Manifest, Profile,
    Protocol, Service,
};

/// The manifest schema version this crate prefers.
pub const SCHEMA_VERSION: u32 = 1;

/// Every schema version this crate can read.
///
/// The reader supports the current generation and exactly one predecessor, so a
/// fork gets one release cycle to catch up without the parser carrying variants
/// indefinitely. Version 1 is the first generation, so it has no predecessor.
pub const SUPPORTED_SCHEMA_VERSIONS: &[u32] = &[SCHEMA_VERSION];

/// Whether a manifest declaring schema `version` can be read by this crate.
///
/// Refusal is loud and names both versions rather than guessing at a layout
/// whose meaning may have shifted. A manifest parsed into the wrong shape fails
/// later and somewhere unrelated, as a confusing Compose error.
#[must_use]
pub fn is_compatible(version: u32) -> bool {
    SUPPORTED_SCHEMA_VERSIONS.contains(&version)
}

#[cfg(test)]
mod tests {
    use super::{is_compatible, SCHEMA_VERSION, SUPPORTED_SCHEMA_VERSIONS};

    #[test]
    fn reads_its_own_schema_version() {
        assert!(is_compatible(SCHEMA_VERSION));
    }

    #[test]
    fn refuses_a_version_it_does_not_support() {
        assert!(!is_compatible(0));
        assert!(!is_compatible(SCHEMA_VERSION + 1));
    }

    #[test]
    fn supports_the_current_generation_and_at_most_one_predecessor() {
        assert!(SUPPORTED_SCHEMA_VERSIONS.contains(&SCHEMA_VERSION));
        assert!(SUPPORTED_SCHEMA_VERSIONS.len() <= 2);
    }
}
