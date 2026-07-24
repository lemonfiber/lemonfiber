//! Parsing and validation of `stack.toml`, the contract between `cli` and
//! `media-stack`. See spec `20-architecture/contracts/stack-manifest.md`.
//!
//! The parser and validator are being built module by module; the schema
//! compatibility gate is the first piece.

/// The manifest schema version this crate can read.
///
/// Implements the compatibility check in `stack-manifest.md` (`F1-R9`).
pub const SCHEMA_VERSION: u32 = 1;

/// Whether a manifest declaring schema `version` can be read by this crate.
///
/// Per `stack-manifest.md` (`F1-R9`) the reader is strict: it reads only its
/// own schema version and refuses anything else, rather than guessing at a
/// newer or older layout. Refusing loudly is safer than importing a manifest
/// whose meaning has shifted.
#[must_use]
pub fn is_compatible(version: u32) -> bool {
    version == SCHEMA_VERSION
}

#[cfg(test)]
mod tests {
    use super::{is_compatible, SCHEMA_VERSION};

    #[test]
    fn reads_its_own_schema_version() {
        assert!(is_compatible(SCHEMA_VERSION));
    }

    #[test]
    fn refuses_any_other_version() {
        assert!(!is_compatible(0));
        assert!(!is_compatible(SCHEMA_VERSION + 1));
    }
}
