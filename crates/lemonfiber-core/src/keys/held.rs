//! The hold one process takes on the record of keys while it reads, changes and writes
//! it.
//!
//! The command line and the serving surface are separate processes, and each changes the
//! record by reading it whole, changing it and writing it back. Two of them doing that at
//! once would each write what the other never read, so a claim could be collected twice
//! and a revoke undone by a mint. Every change is made under this hold, which the
//! operating system gives one open file at a time, and is let go when the change is
//! written.
//!
//! The hold is a file of its own beside the record, because the record is replaced by a
//! rename on every write and a hold on it would be a hold on a file no longer there.

use std::path::Path;

use super::Unkept;
use crate::config::held::Hold;
use crate::config::store;

/// The record of keys, held by this process until this is dropped.
#[derive(Debug)]
pub(crate) struct Held {
    _hold: Hold,
}

impl Held {
    /// The hold on the keys kept at `path`, waited for off the async executor.
    ///
    /// # Errors
    ///
    /// [`Unkept::NotWritten`] where the file the hold is taken on cannot be opened or
    /// held.
    pub(crate) async fn taken(path: &Path) -> Result<Self, Unkept> {
        let opening = path.to_path_buf();
        tokio::task::spawn_blocking(move || Hold::taken(&opening))
            .await
            .map_err(std::io::Error::other)
            .and_then(|held| held)
            .map(|hold| Self { _hold: hold })
            .map_err(|err| {
                Unkept::NotWritten(store::Failure::NotWritten {
                    path: crate::config::held::beside(path),
                    reason: err.to_string(),
                })
            })
    }
}

#[cfg(test)]
mod tests;
