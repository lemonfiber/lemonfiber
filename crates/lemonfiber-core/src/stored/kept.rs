//! Each thing lemonfiber keeps, in the operator's words.
//!
//! Keyed by the accessor on [`Paths`] that names the location, because that is what
//! the guard at the bottom reads: the layout is the list of places, and an entry
//! here is somebody having said what a place is for in a sentence an operator can
//! act on. A place the layout gains and this does not is red.

use crate::config::paths::Paths;

use super::Kept;

/// One thing kept: the accessor that locates it, what it is, why, and whether it
/// holds a credential.
pub struct Entry {
    /// The accessor on the layout that names this location, spelled as it is
    /// written there — which is what the guard below reads the layout for.
    pub accessor: &'static str,
    /// What it is, in the operator's words.
    pub what: &'static str,
    /// Why it is kept.
    pub why: &'static str,
    /// Whether it holds a credential.
    pub secret: bool,
    /// Where it is, given a layout.
    pub at: fn(&Paths) -> std::path::PathBuf,
}

impl Entry {
    /// This entry against a real layout.
    pub(super) fn against(&self, paths: &Paths) -> Kept {
        Kept {
            what: self.what.to_owned(),
            at: (self.at)(paths).display().to_string(),
            why: self.why.to_owned(),
            secret: self.secret,
        }
    }
}

mod every;
mod outside;
mod roots;

pub use every::EVERY;
pub use outside::beside;
pub(super) use roots::roots;

#[cfg(test)]
mod tests;
