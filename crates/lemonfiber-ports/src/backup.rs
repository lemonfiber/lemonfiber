//! What an archive holds, in the words the boundary carries it in.
//!
//! An item to put in, the manifest that lists what went in, and an archive already on
//! disk. Data rather than logic: what to back up, when, and what a restore does with it
//! all live above this, and stay there.
//!
//! Here rather than above the boundary because the archive port speaks in these terms, and
//! a port that could not name what it writes would push the naming into every caller.

use std::path::PathBuf;

/// One thing a capture copies into the archive.
///
/// The source is where it is read from on this machine; the archive path is where
/// it lands inside the archive, stable across machines so a restore knows where to
/// put it back. The label is what a contents listing shows the operator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// Where it is read from.
    pub source: PathBuf,
    /// Where it sits inside the archive, and is restored back to a data root.
    pub archive_path: String,
    /// What a listing calls it, in the operator's terms.
    pub label: String,
}

/// The record written inside an archive, and read back to decide a restore.
///
/// Everything a restore needs to know before it overwrites anything: what made
/// the archive, when, what data root it was taken against, what it covers, whether
/// it is sensitive, and the contents to list. Round-trips through JSON so the same
/// value the capture wrote is the value the restore reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// The archive format, checked before anything inside is trusted.
    pub schema: u32,
    /// The lemonfiber version that wrote it, checked against the one restoring.
    pub product_version: String,
    /// When it was taken. Opaque here; the surface stamps it from the clock.
    pub created_at: String,
    /// The data root it was taken against, to notice a restore to a different one.
    pub data_root: String,
    /// What it covers.
    pub scope: Scope,
    /// Whether it carries credentials, and so must be handled as sensitive.
    pub sensitive: bool,
    /// What is inside, for a listing shown before anything is overwritten.
    pub members: Vec<Member>,
}

/// One entry in an archive's contents listing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    /// Where it sits inside the archive.
    pub archive_path: String,
    /// What it is, in the operator's terms.
    pub label: String,
}

/// One backup already on disk, as retention sees it: a name and when it was taken.
///
/// Ordered by when it was taken, oldest first, so the ones to prune are simply the
/// ones the keep-count does not reach.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Existing {
    /// What the archive is called on disk.
    pub name: String,
    /// When it was taken, compared lexically — the surface names archives so this
    /// holds.
    pub created_at: String,
}
