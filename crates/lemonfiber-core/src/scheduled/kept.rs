//! What a run-and-exit service's runs left behind, one record per service, and the lock
//! one run holds while it goes.

use std::fs::{File, TryLockError};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::instant::Instant;

/// How many of the last lines a run wrote are kept.
pub(crate) const LINES: usize = 20;

/// One run of a run-and-exit service, as it ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ScheduledRun {
    /// When it started.
    pub started: Instant,
    /// When it ended.
    pub ended: Instant,
    /// How it exited, absent where it never started or a signal ended it.
    pub exit: Option<i32>,
    /// The last lines it wrote, oldest first, each with any credential in it withheld;
    /// none to a key, a member or the shared stream, none of which is handed them.
    pub log: Vec<String>,
}

impl ScheduledRun {
    /// The same run with none of the lines it wrote.
    #[must_use]
    pub fn unlogged(self) -> Self {
        Self {
            log: Vec::new(),
            ..self
        }
    }

    /// Whether it exited zero.
    #[must_use]
    pub const fn succeeded(&self) -> bool {
        matches!(self.exit, Some(0))
    }
}

/// What one service's runs left behind.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Kept {
    /// The last minute a run accounted for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) considered: Option<Instant>,
    /// Whether one run is owed whatever its times say.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub(crate) owed: bool,
    /// Its last run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) last: Option<ScheduledRun>,
    /// How many times fell due while a run was still going.
    #[serde(default)]
    pub(crate) skipped: u32,
    /// The last of those.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) last_skipped: Option<Instant>,
}

/// Where `service`'s record is kept under `directory`.
pub(crate) fn record(directory: &Path, service: &str) -> PathBuf {
    directory.join(format!("{service}.json"))
}

/// What `service`'s runs left under `directory`, or nothing kept where none did.
pub(crate) fn read(directory: &Path, service: &str) -> Kept {
    crate::app::record::kept(Some(&record(directory, service)))
}

/// Keep `kept` as `service`'s record under `directory`.
///
/// # Errors
///
/// Where the directory cannot be made or the record cannot be written.
pub(crate) fn write(
    directory: &Path,
    service: &str,
    kept: &Kept,
) -> Result<(), Box<crate::error::Problem>> {
    std::fs::create_dir_all(directory).map_err(|why| {
        Box::new(crate::error::Diagnose::problem(
            &crate::config::store::Failure::NotWritten {
                path: directory.to_path_buf(),
                reason: why.to_string(),
            },
        ))
    })?;
    crate::app::record::keep(Some(&record(directory, service)), kept)
}

/// Mark every one of `services` owed one run under `directory`.
pub(crate) fn owe(directory: &Path, services: &[String]) {
    for service in services {
        let mut kept = read(directory, service);
        if !kept.owed {
            kept.owed = true;
            let _ = write(directory, service, &kept);
        }
    }
}

/// The last `LINES` lines of `said`, each with any credential in it withheld.
pub(crate) fn tail(said: &str) -> Vec<String> {
    let lines: Vec<&str> = said
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    lines
        .iter()
        .skip(lines.len().saturating_sub(LINES))
        .map(|line| crate::config::store::withheld(line))
        .collect()
}

/// The lock one run of `service` holds under `directory`, or nothing where another
/// run holds it.
///
/// # Errors
///
/// Where the lock file cannot be opened.
pub(crate) fn locked(directory: &Path, service: &str) -> std::io::Result<Option<File>> {
    std::fs::create_dir_all(directory)?;
    let file = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(directory.join(format!("{service}.lock")))?;
    match file.try_lock() {
        Err(TryLockError::WouldBlock) => Ok(None),
        locked => locked.map(|()| Some(file)).map_err(std::io::Error::from),
    }
}

#[cfg(test)]
mod tests;
