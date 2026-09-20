//! Parsing of `plugin.toml`, and the two vocabularies a plugin is written against.
//! See spec `20-architecture/contracts/plugin-manifest.md`.
//!
//! This crate is deliberately inert, as `lemonfiber-manifest` is: it reads a
//! manifest into types, refuses one it cannot read, and holds the published sets a
//! manifest names things from. Deciding what to *do* with a plugin belongs to
//! `lemonfiber-core`, which is what lets both be tested with no Docker, no terminal
//! and no configuration.
//!
//! The three artefacts this crate is the source of are generated from these types
//! rather than written beside them — the schema from [`Manifest`], the capability
//! vocabulary from [`vocabulary`], the extension points from [`extension`] — so a
//! shape that changes without its artefact changing with it fails the build.

pub mod claiming;
mod conforming;
mod error;
pub mod extension;
pub mod offering;
pub mod pointing;
pub mod refusing;
mod schema;
pub mod vocabulary;

pub use conforming::Violation;
pub use error::Error;
pub use refusing::refusals;
pub use schema::{
    Bind, Capture, Claim, ClaimProbe, Contribution, Criticality, Entry, Expect, Expected, Health,
    HealthKind, Kind, Manifest, Override, Pair, Plugin, Proof, Recipe, Request, Requires, Secret,
    Service, Step, StepCall, Wiring, CONFIGURATION, RUN,
};

/// The manifest schema version this crate prefers.
pub const SCHEMA_VERSION: u32 = 1;

/// Every schema version this crate can read.
///
/// A plugin generation is supported until it is deprecated by announcement and then
/// removed, never merely by being overtaken — which is where this parts company with
/// the stack manifest's window of the current generation and one predecessor. A fork's
/// maintainer is one person actively tracking a release cycle; a plugin ecosystem is
/// mostly authors who are not watching, and a plugin that was finished is not
/// abandoned. Deleting the long tail of the catalogue at every second bump is not
/// experienced as "this plugin is old" but as "my media server stopped".
pub const SUPPORTED_SCHEMA_VERSIONS: &[u32] = &[SCHEMA_VERSION];

/// Whether a manifest declaring schema `version` can be read by this crate.
///
/// Refusal is loud and names both versions rather than guessing at a layout whose
/// meaning may have shifted. A manifest parsed into the wrong shape fails later and
/// somewhere unrelated, as a confusing failure to install.
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
    fn supports_every_generation_it_has_ever_published() {
        assert!(SUPPORTED_SCHEMA_VERSIONS.contains(&SCHEMA_VERSION));
    }
}
