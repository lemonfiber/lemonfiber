//! Telling a hand-edit to the configuration file from what lemonfiber put there.
//!
//! The environment file is a file an operator edits, deliberately: comments and
//! ordering are preserved on every write for exactly that reason. So a change made
//! through lemonfiber may be about to write over one made outside it, and the two
//! are indistinguishable from the file alone — the value is simply a value.
//!
//! Three values tell them apart where two cannot: what lemonfiber last wrote here,
//! what the file holds now, and what is about to be written. That is the same
//! comparison seeding makes about a service's configuration, so it is the same
//! comparison — reached for rather than written again, because two answers to
//! "whose value is this" that could disagree is worse than none.

use crate::baseline::Record;
use crate::seed::{reconcile, Observed};

/// What the record, the file and the change together say about a setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// The file already holds what is about to be written; there is nothing to do.
    Already,
    /// The file holds what lemonfiber last wrote there, so the change is
    /// lemonfiber's own to make.
    Ours,
    /// lemonfiber has no record of ever writing here, so an operator's edit cannot
    /// be told from the value setup itself left. What is there is taken as the
    /// starting point rather than judged against an expectation nobody formed.
    Unrecorded,
    /// The file was changed outside lemonfiber since it last wrote here.
    Edited,
}

/// What `writing` would be doing to this setting, given what lemonfiber recorded
/// and what the file holds now.
///
/// A setting missing from the file that lemonfiber has a record for reads as an
/// edit, because it is one: somebody took the line out.
#[must_use]
pub fn standing(recorded: Option<&Record>, found: Option<&str>, writing: &str) -> Standing {
    match reconcile(recorded, found, writing) {
        Observed::Present => Standing::Already,
        Observed::Stale => Standing::Ours,
        Observed::Unmanaged => Standing::Unrecorded,
        _ => Standing::Edited,
    }
}

#[cfg(test)]
mod tests;
