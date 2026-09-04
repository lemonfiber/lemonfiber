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
mod tests {
    use std::path::PathBuf;

    use super::{tally, Counting};
    use crate::ports::filesystem::Identity;
    use crate::ports::occupancy::Occupant;

    /// A file of a given size, under a given inode with a given number of names.
    fn file(path: &str, bytes: u64, inode: u64, links: u64) -> Occupant {
        Occupant {
            path: PathBuf::from(path),
            bytes,
            identity: Some(Identity { file: inode, links }),
        }
    }

    /// A file the platform would not identify.
    fn nameless(path: &str, bytes: u64) -> Occupant {
        Occupant {
            path: PathBuf::from(path),
            bytes,
            identity: None,
        }
    }

    #[test]
    fn a_file_reachable_under_two_names_is_paid_for_once() {
        let counted = tally(&[
            file("/data/downloads/a.mkv", 8_000, 41, 2),
            file("/data/media/a.mkv", 8_000, 41, 2),
        ]);
        assert_eq!(counted.logical, 16_000, "what the names add up to");
        assert_eq!(counted.physical, 8_000, "what the disk actually lost");
        assert_eq!(counted.files, 2);
        assert_eq!(counted.shared, 1);
        assert!(counted.differs());
        assert_eq!(counted.saved(), 8_000);
    }

    #[test]
    fn nothing_shared_reads_as_one_figure_rather_than_two() {
        let counted = tally(&[
            file("/data/media/a.mkv", 8_000, 41, 1),
            file("/data/media/b.mkv", 2_000, 42, 1),
        ]);
        assert_eq!(counted.logical, 10_000);
        assert_eq!(counted.physical, 10_000);
        assert_eq!(counted.shared, 0);
        assert!(!counted.differs(), "one figure, not two of the same");
        assert_eq!(counted.saved(), 0);
    }

    #[test]
    fn a_file_the_platform_would_not_identify_is_charged_in_full() {
        // Two unidentified files of the same size are not evidence of one file, and
        // a total short by a real file is worse than one long by an unread one.
        let counted = tally(&[
            nameless("/data/a.mkv", 5_000),
            nameless("/data/b.mkv", 5_000),
        ]);
        assert_eq!(counted.physical, 10_000);
        assert_eq!(counted.shared, 0);
    }

    #[test]
    fn a_zero_identity_is_no_identity() {
        let counted = tally(&[
            file("/data/a.mkv", 5_000, 0, 1),
            file("/data/b.mkv", 5_000, 0, 1),
        ]);
        assert_eq!(
            counted.physical, 10_000,
            "two zeroes are not evidence of one file"
        );
        assert_eq!(counted.shared, 0);
    }

    #[test]
    fn a_file_shared_between_two_trees_is_charged_to_the_first_of_them() {
        // The reason the count is held across trees rather than restarted per tree:
        // each tree counted alone is right about itself, and the sum of them is
        // twice the truth.
        let mut counting = Counting::default();
        let downloads = counting.count(&[file("/data/downloads/a.mkv", 8_000, 41, 2)]);
        let library = counting.count(&[file("/data/media/a.mkv", 8_000, 41, 2)]);
        assert_eq!(downloads.physical, 8_000);
        assert_eq!(library.physical, 0, "the second tree pays nothing again");
        assert_eq!(library.shared, 1);
        assert_eq!(
            downloads.physical + library.physical,
            8_000,
            "the volume lost eight thousand bytes, not sixteen"
        );
    }

    #[test]
    fn counting_nothing_comes_to_nothing() {
        let counted = tally(&[]);
        assert_eq!(counted.logical, 0);
        assert_eq!(counted.physical, 0);
        assert_eq!(counted.files, 0);
    }

    #[test]
    fn a_size_large_enough_to_wrap_saturates_instead() {
        let counted = tally(&[
            file("/data/a.mkv", u64::MAX, 41, 1),
            file("/data/b.mkv", u64::MAX, 42, 1),
        ]);
        assert_eq!(
            counted.logical,
            u64::MAX,
            "held at the top rather than wrapped"
        );
        assert_eq!(counted.physical, u64::MAX);
    }
}
