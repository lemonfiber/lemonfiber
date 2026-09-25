//! Counting bytes once, however many names point at them.
//!
//! On a correctly configured stack a file lives in the downloads tree and in the
//! library at the same time: the import made a second name for it rather than a
//! second copy. Adding the two directory listings together says the disk holds
//! twice what it holds, and every figure built on that sum — what is free, what
//! would be reclaimed, which tree is growing — is wrong by the same amount.
//!
//! So the sum is taken over underlying files rather than over names. Both figures
//! are kept, because they answer different questions: what the tree would take on
//! a filesystem that could not link is what an operator is quoted when they think
//! about moving it, and what it actually occupies is what the volume has lost.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::ports::occupancy::Occupant;

/// What a set of files occupies, counted both ways.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Tally {
    /// The bytes the names add up to — what this would take with nothing shared.
    pub logical: u64,
    /// The bytes the underlying files add up to — what the volume has actually
    /// lost to them.
    pub physical: u64,
    /// How many names were counted.
    pub files: usize,
    /// How many of those names pointed at a file already counted.
    pub shared: usize,
}

impl Tally {
    /// Whether the two figures differ, which is the only case worth reporting both.
    ///
    /// Where nothing is shared they are the same number, and printing it twice
    /// under two headings invites an operator to look for a difference that is not
    /// there.
    #[must_use]
    pub const fn differs(&self) -> bool {
        self.logical != self.physical
    }

    /// What the sharing saved — the bytes a copy-mode stack would have spent extra.
    #[must_use]
    pub const fn saved(&self) -> u64 {
        self.logical.saturating_sub(self.physical)
    }
}

/// A running count that remembers which underlying files it has already paid for.
///
/// Held across several trees rather than restarted per tree, because the file that
/// two trees share is the whole reason this exists: counted separately each tree is
/// right about itself and the sum of them is wrong. The tree the counting reaches
/// first is the one charged for a shared file, which is why the order trees are
/// counted in is the order they are reported in — an arbitrary split of a shared
/// cost, made visible rather than hidden.
#[derive(Debug, Default)]
pub struct Counting {
    /// The underlying files already paid for.
    seen: BTreeSet<u64>,
}

impl Counting {
    /// Count these occupants, charging for each underlying file the first time it
    /// is met.
    ///
    /// An occupant whose identity could not be read is charged in full every time,
    /// because nothing establishes that it is the same file as another — and a
    /// figure that is too large by an unread file is better than one that is too
    /// small by a real one.
    pub fn count(&mut self, occupants: &[Occupant]) -> Tally {
        let mut tally = Tally {
            files: occupants.len(),
            ..Tally::default()
        };
        for occupant in occupants {
            tally.logical = tally.logical.saturating_add(occupant.bytes);
            let counted = match occupant.identity {
                Some(identity) if identity.file != 0 => self.seen.insert(identity.file),
                // A platform that reports no file number leaves nothing to compare,
                // and two zeroes are not evidence of one file however equal they look.
                _ => true,
            };
            if counted {
                tally.physical = tally.physical.saturating_add(occupant.bytes);
            } else {
                tally.shared += 1;
            }
        }
        tally
    }
}

/// What one set of files occupies, counted on its own.
///
/// For a tree measured apart from every other, where a file reachable under two
/// names inside it must still be paid for once.
#[must_use]
pub fn tally(occupants: &[Occupant]) -> Tally {
    Counting::default().count(occupants)
}

#[cfg(test)]
mod tests;
