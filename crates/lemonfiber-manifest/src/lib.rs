//! Parsing and validation of `stack.toml`, the contract between `cli` and
//! `media-stack`. See spec `20-architecture/contracts/stack-manifest.md`.
//!
//! Skeleton — the parser and validator are not yet implemented.

/// The manifest schema version this crate can read.
///
/// Implements the compatibility check in `stack-manifest.md` (`F1-R9`).
pub const SCHEMA_VERSION: u32 = 1;
