//! The stack this build embeds, as the one manifest text its files assemble into.
//!
//! `assets/media-stack` describes itself in a root and a file per service
//!. Everything compiled against the bundled stack reads it through
//! here, so none of it has to know the stack is written in pieces, and the build
//! fails before any of it compiles when the files break the rules about them.

/// The embedded stack's manifest: its root, then each service's file in the order
/// `include` lists them.
pub const STACK: &str = include_str!(concat!(env!("OUT_DIR"), "/stack.toml"));
