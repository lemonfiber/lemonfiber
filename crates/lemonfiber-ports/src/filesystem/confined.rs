//! Reading and writing a file somebody else's container can write, and asking where a
//! path it can reach lies, from code that cannot wait.

use std::path::Path;

use super::{Beneath, Fault};

/// Reading and writing, from code that cannot wait, a file somebody else's container can
/// write.
///
/// The same promise [`super::FileSystem::read_beneath`] makes, kept by the same
/// implementation, for the callers that write the stack's own files out in one pass with
/// nothing to await on the way. A trait of its own for the reason [`super::Eraser`] is one:
/// those callers need nothing else of a filesystem.
pub trait Confined: Send + Sync {
    /// Read `path`, only where it is a plain file beneath `within` once every link on the
    /// way is resolved, as [`super::FileSystem::read_beneath`] reads it.
    fn read(&self, path: &Path, within: &Path) -> Beneath;

    /// Write `contents` into `path` in place, creating it where it is not there, only
    /// where it is a plain file beneath `within` once every link on the way is resolved.
    ///
    /// In place, because a container given one file follows that file rather than
    /// whatever replaces it.
    ///
    /// # Errors
    ///
    /// Returns a [`Fault`] where a link, a path leading outside `within` or something that
    /// is not a plain file is where the file is expected, or where the write itself fails.
    fn overwrite(&self, path: &Path, within: &Path, contents: &[u8]) -> Result<(), Fault>;

    /// Whether `path` lies beneath `within` with no link at any step on the way, itself
    /// included, whatever it is.
    fn lies(&self, path: &Path, within: &Path) -> Lying;

    /// Move `from` to `to`, both beneath `within` with no link at any step on the way to
    /// either, the two themselves included, making the directories missing above `to` the
    /// same way, and never over anything already at `to`.
    ///
    /// # Errors
    ///
    /// Returns a [`Fault`] where a link, a path leading outside `within`, nothing at `from`
    /// or something already at `to` is met, or where the move itself fails; nothing is
    /// moved.
    fn moved(&self, from: &Path, to: &Path, within: &Path) -> Result<(), Fault>;
}

/// Where a path somebody else's container can write lies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lying {
    /// Beneath the directory, reached through no link.
    Plain,
    /// Not there, every step that is there reached through no link.
    Absent,
    /// A link on the way or at its name, or not beneath the directory at all.
    Escaped,
}
