//! Parsing of `stack.toml`, the contract between `lemonfiber` and `lemonfiber-media-stack`.
//! See spec `20-architecture/contracts/stack-manifest.md`.
//!
//! This crate is deliberately inert: it reads a manifest into types and refuses
//! one it cannot read. Deciding what to *do* with a manifest belongs to
//! `lemonfiber-core`, which is what lets both be tested with no Docker, no
//! terminal and no configuration.

mod date;
mod error;
mod recognising;
mod schema;
mod validate;

pub use date::Date;
pub use error::Error;
pub use schema::{
    Api, ApiKind, Bind, Criticality, Form, Health, HealthKind, KeySource, Manifest, Profile,
    Protocol, Removed, Service,
};
pub use validate::{is_core_name, validate, Violation, ALLOWED_GRANTS};

/// The manifest schema version this crate prefers.
pub const SCHEMA_VERSION: u32 = 1;

/// Every schema version this crate can read.
///
/// The reader supports the current generation and exactly one predecessor, so a
/// fork gets one release cycle to catch up without the parser carrying variants
/// indefinitely. Version 1 is the first generation, so it has no predecessor.
///
/// A table added to the contract does not move this, and the record of what a stack
/// has dropped is the case that settled it. The generation is about fields *meaning
/// something different* — that is what a reader cannot recover from, because it parses
/// a manifest into the wrong shape and fails later and somewhere unrelated. A table an
/// older reader has never heard of is not that: nothing it already understands changes
/// meaning, and the manifest is refused outright rather than misread. What refuses it
/// is `deny_unknown_fields`, which is a strictness choice about unknown *names*, not a
/// statement about the generation.
///
/// Moving the generation for it would cost more than it bought. An older binary
/// refuses such a manifest either way, so the only thing gained is the wording of the
/// refusal — and the price is that every already-shipped copy refuses the whole of a
/// newer stack, including the eighteen services that did not change. It would also
/// spend the entire compatibility window, which is one generation wide, on a table
/// that records history; the next change that really does make a field mean something
/// else would then have to drop generation 1 to make room for itself.
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
