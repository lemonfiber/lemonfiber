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

use std::fs::File;
use std::path::{Path, PathBuf};

use super::Unkept;
use crate::config::store;

/// What is put after the record's name to name the file the hold is taken on.
const SUFFIX: &str = ".lock";

/// The record of keys, held by this process until this is dropped.
#[derive(Debug)]
pub(crate) struct Held(File);

impl Held {
    /// The hold on the keys kept at `path`, waited for off the async executor.
    ///
    /// # Errors
    ///
    /// [`Unkept::NotWritten`] where the file the hold is taken on cannot be opened or
    /// held.
    pub(crate) async fn taken(path: &Path) -> Result<Self, Unkept> {
        let at = beside(path);
        let opening = at.clone();
        tokio::task::spawn_blocking(move || locked(&opening))
            .await
            .map_err(std::io::Error::other)
            .and_then(|locked| locked)
            .map(Self)
            .map_err(|err| {
                Unkept::NotWritten(store::Failure::NotWritten {
                    path: at,
                    reason: err.to_string(),
                })
            })
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

/// The file the hold on the record at `path` is taken on.
fn beside(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(SUFFIX);
    path.with_file_name(name)
}

/// The file at `at`, made where it is not there yet and held once nothing else holds it.
fn locked(at: &Path) -> std::io::Result<File> {
    let parent = at
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    store::make_private_dir(parent)?;
    let file = opened(at)?;
    file.lock()?;
    Ok(file)
}

/// The file at `at`, owner-only where it is made, opened through no link and waiting on
/// no pipe.
#[cfg(unix)]
fn opened(at: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt as _;
    File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(at)
}

/// The file at `at`, where the platform has no owner-only mode or link flag to honour.
#[cfg(not(unix))]
fn opened(at: &Path) -> std::io::Result<File> {
    File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(at)
}

#[cfg(test)]
mod tests;
