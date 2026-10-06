//! Putting a configuration back from a backup archive.
//!
//! Its own file beside the command that carries it, the way every other value a
//! command carries has one.

use super::super::restore;

/// Which archive to restore from, and what this run was given consent for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restoring {
    /// The archive to restore from, named the way the surface can name one.
    pub archive: restore::Kept,
    /// Whether re-pointing to this machine's data root was accepted.
    pub repoint: bool,
    /// How much of the restore this run was given consent for, and for which
    /// listing. Without a yes the archive is verified and its contents listed,
    /// and nothing is touched.
    pub consent: restore::Consent,
}
