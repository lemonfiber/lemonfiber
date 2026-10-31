//! Parsing of a stack manifest: `stack.toml` and the service files it includes, as the
//! bundle assembles them or a `--stack-dir` stack holds them.
//! See spec `20-architecture/contracts/stack-manifest.md`.
//!
//! This crate is deliberately inert: it reads a manifest into types and refuses
//! one it cannot read. Deciding what to *do* with a manifest belongs to
//! `lemonfiber-core`, which is what lets both be tested with no Docker, no
//! terminal and no configuration.

pub mod assembly;
mod binary;
pub mod container;
mod date;
mod error;
pub mod names;
mod recognising;
mod schema;
mod spelling;
mod validate;

pub use assembly::{assemble, read};
pub use date::Date;
pub use error::Failure;
pub use schema::{
    majors, ApiKind, Bind, Claimed, Criticality, Dashboard, Form, Health, HealthKind, Manifest,
    Profile, Protocol, Removed, Service, Wiring, CLAIM,
};
pub use spelling::environment_name;
pub use validate::{is_core_name, is_digest, validate, Violation, GATEWAY_GRANT, TUNNEL_DEVICE};

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

/// A day the bundled stack is not ahead of: 2030-01-01T00:00:00Z, in seconds since the
/// Unix epoch.
///
/// A test that validates the manifest is validating it against whatever day its
/// clock says it is, and a service records the day its upstream last released — so
/// a clock behind the manifest reads those dates as being in the future and fails
/// validation. The manifest is a submodule that moves forward and these clocks do
/// not, so every one of them reads this day rather than each going stale on its own.
///
/// `the_day_these_tests_use_is_after_everything_the_stack_records` is the guard. When
/// it fails, move this forward; do not pin an older image to satisfy it.
#[cfg(any(test, feature = "testing"))]
pub const TODAY: u64 = 1_893_456_000;

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
mod tests;
