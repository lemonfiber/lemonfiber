//! The one file a configuration backup lives in.
//!
//! What a capture *includes* is decided in [`crate::backup`], with no disk in
//! sight; what it takes to turn that decision into a portable file — walking the
//! chosen trees, packing them beside the manifest, measuring the room they need,
//! listing and pruning the archives already there — is the outside world, and is
//! reached only through here. The capture executor above drives these operations
//! and decides what each failure means, so the whole of its policy stays testable
//! with a fake in place of a real archive on a real disk.
//!
//! The read and extract side a restore needs is added with the restore executor;
//! this is the write and retention side a capture needs.

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::backup::{Existing, Item, Manifest};

/// The room a capture would take, against what is free where it would be written.
///
/// Both figures come from one reading so they describe the same moment: a capture
/// checked against a free-space figure measured a second apart could still fill a
/// disk that filled between the two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Space {
    /// The bytes the captured trees are estimated to occupy.
    pub needed: u64,
    /// The bytes free on the volume the archive would be written to.
    pub available: u64,
}

impl Space {
    /// Whether the capture fits with `headroom` bytes still to spare.
    ///
    /// Headroom because an archive is written into the same space it is measured
    /// against, and a capture that used the last free byte would leave the volume
    /// it was meant to protect with nothing to work in.
    #[must_use]
    pub const fn fits(&self, headroom: u64) -> bool {
        self.available >= self.needed.saturating_add(headroom)
    }
}

/// An archive operation could not be completed, in the platform's own words.
///
/// One shape for every refusal, as with a filesystem fault: the executor decides
/// what a failure *at a given step* means — a write that fails stops the capture,
/// a prune that fails does not — so the distinction is about which call failed,
/// not about how the operating system worded it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fault {
    /// The platform's own description of what went wrong.
    pub message: String,
}

impl Fault {
    /// A fault carrying the platform's description of it.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Writes, measures, lists and prunes backup archives.
///
/// Each method is one operation the capture executor sequences, so the executor's
/// ordering — measure, then write, then prune the surplus — is decided above the
/// port where a fake drives every branch of it, and only the packing and the
/// disk reads themselves are left to the one real implementation.
#[async_trait]
pub trait Archive: Send + Sync {
    /// Measure the room `items` would take against what is free at `dir`.
    ///
    /// # Errors
    ///
    /// Returns a [`Fault`] where the trees could not be walked or the free space
    /// not read.
    async fn space(&self, dir: &Path, items: &[Item]) -> Result<Space, Fault>;

    /// Write an archive at `dest` holding `manifest` and each item's tree.
    ///
    /// Atomic against failure: `dest` is left either the complete archive or
    /// absent, never a truncated one — written to a temporary name and renamed
    /// into place, so a stop part-way leaves nothing a later listing would mistake
    /// for a good backup. An archive already at `dest` is not overwritten; a
    /// same-named capture is refused rather than silently replacing it.
    ///
    /// # Errors
    ///
    /// Returns a [`Fault`] where the archive could not be created or written, or
    /// where one already exists at `dest`.
    async fn write(&self, dest: &Path, manifest: &Manifest, items: &[Item]) -> Result<(), Fault>;

    /// Write an archive at `dest` holding these named files, whose contents are in hand
    /// rather than on disk.
    ///
    /// A support bundle is generated, not copied: its files are a diagnosis rendered, a
    /// configuration redacted, a page describing the rest. None of them exist anywhere to
    /// be pointed at, and staging them on disk to be picked up again would put the
    /// redacted contents somewhere they would have to be cleaned up from afterwards.
    ///
    /// Atomic against failure and refusing an existing `dest`, exactly as a capture is,
    /// and for the same reason: a bundle half-written is a bundle somebody sends.
    ///
    /// # Errors
    ///
    /// Returns a [`Fault`] where the archive could not be created or written, or where
    /// one already exists at `dest`.
    async fn write_files(&self, dest: &Path, files: &[(String, String)]) -> Result<(), Fault>;

    /// The backups already present in `dir`, for retention to prune from.
    ///
    /// Includes an archive written moments earlier, and gives each a `created_at`
    /// that sorts the most recent last, so retention run just after a capture sees
    /// the fresh archive as the newest and never the oldest to prune.
    ///
    /// # Errors
    ///
    /// Returns a [`Fault`] where the directory could not be read.
    async fn existing(&self, dir: &Path) -> Result<Vec<Existing>, Fault>;

    /// Remove the pruned backup `name` from `dir`.
    ///
    /// # Errors
    ///
    /// Returns a [`Fault`] where the archive could not be removed.
    async fn remove(&self, dir: &Path, name: &str) -> Result<(), Fault>;
}

/// Reads a backup archive back — its manifest, and its contents onto disk.
///
/// A trait of its own rather than more methods on [`Archive`], as with a
/// filesystem's watch: a restore reads and never writes a backup, a capture
/// writes and never reads one, and keeping them apart means each side is driven in
/// a test by a fake that answers only the question it is asked.
#[async_trait]
pub trait Reader: Send + Sync {
    /// Read the manifest from the archive at `src`, without unpacking it.
    ///
    /// This is the verification a restore does before it overwrites anything: an
    /// archive whose manifest cannot be read is corrupt, and is refused here
    /// rather than discovered half-way through a restore.
    ///
    /// # Errors
    ///
    /// Returns a [`Fault`] where the archive is missing, unreadable, or does not
    /// hold a manifest this build understands.
    async fn read_manifest(&self, src: &Path) -> Result<Manifest, Fault>;

    /// Unpack the archive at `src`, writing each member under the directory its
    /// area names in `targets`.
    ///
    /// Sanitises every real entry itself, independently of any manifest: the
    /// manifest is read from the same untrusted archive and its member list need
    /// not match the bytes beside it, so a caller's manifest-level check is no
    /// guarantee about what unpacking actually writes. Each entry whose path
    /// contains `..`, is absolute, or is a symlink or hardlink pointing outside its
    /// target is refused rather than followed — the archive-traversal footgun a
    /// self-describing manifest cannot close.
    ///
    /// An entry whose leading area is not in `targets` is not written to a guessed
    /// place; a build that meets an area it does not know reports it rather than
    /// silently dropping it, so a partial restore is never mistaken for a complete
    /// one.
    ///
    /// Atomic against failure, as [`Archive::write`] is: it stages the contents and
    /// swaps them into place, so a fault part-way leaves the prior configuration
    /// intact rather than half-overwritten — a restore that fails is worse than one
    /// that refuses to start. Where a swap is not possible, any partial state left
    /// is reconciled by the seed a restore is always followed by.
    ///
    /// # Errors
    ///
    /// Returns a [`Fault`] where the archive could not be read, an entry would
    /// escape its target, an area is unrecognised, or a member could not be
    /// written.
    async fn extract(&self, src: &Path, targets: &[(String, PathBuf)]) -> Result<(), Fault>;
}

#[cfg(test)]
mod tests {
    use super::{Fault, Space};

    #[test]
    fn a_capture_fits_only_when_the_headroom_is_left_over() {
        let space = Space {
            needed: 100,
            available: 250,
        };
        assert!(space.fits(150), "250 holds 100 with 150 to spare");
        assert!(!space.fits(151), "250 cannot hold 100 and 151 more");
    }

    #[test]
    fn a_headroom_that_would_overflow_does_not_wrap_into_fitting() {
        // needed + headroom must saturate rather than wrap past the available
        // figure and read as room where there is none: a gigantic estimate on a
        // small disk does not fit, however large the headroom added to it.
        let space = Space {
            needed: u64::MAX,
            available: 1_000,
        };
        assert!(!space.fits(1));
    }

    #[test]
    fn a_fault_keeps_the_platforms_own_words() {
        assert_eq!(Fault::new("disk full").message, "disk full");
    }
}
