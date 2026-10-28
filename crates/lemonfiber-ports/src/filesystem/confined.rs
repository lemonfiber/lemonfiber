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

    /// Write `contents` whole into `path`, making it and every directory missing above it
    /// where they are not there, as `readable` says, only where it and every step to it
    /// lie beneath `within` with no link on the way.
    ///
    /// # Errors
    ///
    /// Returns a [`Fault`] where a link, a path leading outside `within` or something that
    /// is not a plain file is met, or where the write itself fails.
    fn written(
        &self,
        path: &Path,
        within: &Path,
        contents: &[u8],
        readable: Readable,
    ) -> Result<(), Fault>;

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

    /// Make the directory `path` with every directory missing above it, beneath `within`,
    /// each step reached through no link; a directory made here is readable as `readable`
    /// says, and where `readable` is [`Readable::Owner`] every step below `within` has to be
    /// a directory of the writer's own.
    ///
    /// # Errors
    ///
    /// Returns a [`Fault`] where a link, a path leading outside `within`, something that
    /// is not a directory or, for [`Readable::Owner`], a directory of somebody else's is
    /// met, or where a directory cannot be made.
    fn made(&self, path: &Path, within: &Path, readable: Readable) -> Result<(), Fault>;

    /// Replace `path` whole with `contents`: written to a file made new beside it through
    /// the handle of the directory it is in, flushed, and renamed over it in one step, the
    /// directories above it made as [`Confined::made`] makes them.
    ///
    /// A reader sees what was there or all of `contents`, never a part, and a link at
    /// `path` is replaced rather than followed.
    ///
    /// # Errors
    ///
    /// Returns a [`Fault`] where [`Confined::made`] refuses the directory, or the file
    /// cannot be written or moved into place; nothing at `path` changes.
    fn replaced(
        &self,
        path: &Path,
        within: &Path,
        contents: &[u8],
        readable: Readable,
    ) -> Result<(), Fault>;

    /// What `path` holds, byte for byte, read through no link at any step and only where
    /// it is a plain file no larger than [`super::READ_LIMIT`], or nothing where it or a
    /// directory above it is not there.
    ///
    /// # Errors
    ///
    /// Returns a [`Fault`] where a link, a path leading outside `within`, something that is
    /// not a plain file or a file past the limit is met, or where it cannot be read.
    fn bytes(&self, path: &Path, within: &Path) -> Result<Option<Vec<u8>>, Fault>;
}

/// Who may read a file written beneath a directory a container can write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Readable {
    /// Its owner alone, in directories of its owner's own: a file of lemonfiber's that holds
    /// a secret.
    Owner,
    /// Its owner alone, in directories the service it is written for may have made: a file
    /// holding a secret, written into a service's own configuration.
    Private,
    /// Whoever the service runs as, as the stack's own files are.
    Shared,
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
