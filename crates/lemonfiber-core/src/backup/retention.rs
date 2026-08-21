//! How many backups to keep.
//!
//! Pruning is the one part of backing up that destroys something, so the rule is
//! kept small enough to read in one sitting — and it never prunes the last archive
//! standing, whatever the operator set the count to.

use super::Existing;

/// How many backups to keep before the oldest are pruned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Retention {
    keep: usize,
}

impl Retention {
    /// Keep `keep` backups, pruning the oldest beyond that — but never the last
    /// one standing.
    ///
    /// A keep of zero is raised to one: retention exists to bound the space
    /// backups take, and a policy that pruned to nothing would delete the very
    /// thing it is meant to preserve, turning a full disk into no recovery at all.
    #[must_use]
    pub const fn keeping(keep: usize) -> Self {
        Self {
            keep: if keep == 0 { 1 } else { keep },
        }
    }

    /// The backups to prune from `existing`, oldest first, leaving `keep` newest.
    ///
    /// Takes the set by value and sorts it, so the caller's order does not decide
    /// which survive — the oldest by their recorded time do, whatever order they
    /// were listed in.
    #[must_use]
    pub fn prune(self, mut existing: Vec<Existing>) -> Vec<Existing> {
        existing.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        let surplus = existing.len().saturating_sub(self.keep);
        existing.truncate(surplus);
        existing
    }
}
