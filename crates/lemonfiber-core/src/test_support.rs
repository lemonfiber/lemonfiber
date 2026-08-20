//! The one fake that names a type from this crate.
//!
//! Everything else a test stands in for lives in `lemonfiber-fixtures`, where the crate's
//! own tests and its integration tests reach the same one. This stayed because it hands
//! back a [`crate::stack::Source`], which is above the boundary that crate depends on.

use crate::stack::Source;

pub(crate) use lemonfiber_fixtures::support::*;

/// The stack this repository carries, read from disk.
pub(crate) fn stack() -> Source {
    Source::External(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/media-stack"
    )))
}
