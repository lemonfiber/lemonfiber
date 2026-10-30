//! The port an address is reached on where it names none, by its scheme.
//!
//! Said once, because every address this product takes apart falls back to the same
//! two, and two places assuming them are two places to assume differently.

/// Where `http://` is reached on.
pub const PLAIN: u16 = 80;

/// Where `https://` is reached on.
pub const SECURE: u16 = 443;

/// The scheme an address is reached over when what travels must be encrypted.
pub const SECURE_SCHEME: &str = "https";
