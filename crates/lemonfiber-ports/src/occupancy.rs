//! Walking a tree to find out what is actually in it.
//!
//! A volume's free space says how much room is left and nothing about where the
//! rest went, which is the question an operator asks the moment they see the
//! figure. Answering it means looking at the files, and looking at the files is
//! the one thing only a real filesystem can do — so it is a seam.
//!
//! What crosses the seam is one entry per file rather than a total, because the
//! sum is the part that has to be decided above it. A file reachable under two
//! names occupies its bytes once, and a port that returned a total would have
//! made that judgement inside the one place a test cannot reach. So the walk
//! reports what it found, identity and all, and the counting happens where a fake
//! can drive every case of it.
//!
//! The cost of that choice is a list rather than a running sum: a library of two
//! hundred thousand files is a few tens of megabytes held while the reckoning is
//! made, and it is held once, for a command somebody asked for rather than a loop
//! that runs every second.
//!
//! See `.docs/architecture/ports-and-adapters.md`.

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::filesystem::{Fault, Identity};

/// One file found beneath a surveyed tree.
///
/// The identity travels with it because two entries that name one file must be
/// counted once, and nothing downstream can establish that from a path and a
/// size. Absent where the platform would not say — an entry whose metadata could
/// not be read is still a file taking up room, and dropping it would understate
/// the total rather than leaving one figure unproven.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Occupant {
    /// Where the file is.
    pub path: PathBuf,
    /// How many bytes it holds, as the platform reports its length.
    pub bytes: u64,
    /// Which underlying file it names, and how many names point at it.
    pub identity: Option<Identity>,
}

/// Surveying what a directory tree holds, file by file.
///
/// A trait of its own rather than another method on the wider filesystem port,
/// for the reason the volume watch and the eraser are: the reckoning that needs
/// this needs nothing else of a filesystem, and every other implementation of
/// that trait would gain a method it never calls.
#[async_trait]
pub trait Occupancy: Send + Sync {
    /// Every file beneath `root`, recursively.
    ///
    /// Directories themselves are not reported: what they occupy is their own
    /// metadata rather than content, and an operator asking where their disk went
    /// is not asking about directory entries. A tree that is not there reports no
    /// files rather than failing, because a data location configured before it was
    /// created is an ordinary state and not one to refuse a reading over.
    ///
    /// # Errors
    ///
    /// Returns a [`Fault`] where the tree could not be walked at all — a root that
    /// exists and cannot be read, which is a permission problem the operator has to
    /// hear about rather than an empty answer to hand them.
    async fn beneath(&self, root: &Path) -> Result<Vec<Occupant>, Fault>;
}
