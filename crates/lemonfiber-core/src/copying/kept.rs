//! The reading a running copy keeps on disk, and the lock that says its copy still runs.

use std::fs::{File, TryLockError};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::{Running, State, What};
use crate::backup::Scope;
use crate::instant::Instant;

/// The reading, inside [`crate::config::paths::Paths::copying`].
const READING: &str = "reading.json";

/// The lock the copy holds for as long as it runs, beside the reading.
const LOCK: &str = "reading.lock";

/// How many times a copy asks for the lock while a reader is checking it.
const TRIES: u32 = 20;

/// How long a copy waits between those asks.
const BETWEEN: Duration = Duration::from_millis(5);

/// The reading of one running copy, as kept on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Kept {
    pub(super) what: What,
    pub(super) scope: Scope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) job: Option<String>,
    pub(super) started: Instant,
    pub(super) written: u64,
    pub(super) whole: u64,
    pub(super) part: String,
    pub(super) wrote: Instant,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) limit: Option<u64>,
}

impl Kept {
    /// The copy as read at `now`, stalled where it has written nothing for `stalled` seconds.
    pub(super) fn running(self, now: Instant, stalled: u64) -> Running {
        let state = if now.since(self.wrote) >= stalled {
            State::Stalled { wrote: self.wrote }
        } else {
            self.limit
                .map_or(State::Working, |limit| State::Paced { limit })
        };
        Running {
            what: self.what,
            scope: self.scope.into(),
            job: self.job,
            started: self.started,
            written: self.written,
            whole: self.whole,
            part: self.part,
            state,
        }
    }
}

/// Where the reading is kept under `dir`.
pub(super) fn reading(dir: &Path) -> PathBuf {
    dir.join(READING)
}

/// The lock under `dir`, held, or nothing where another copy holds it or it cannot be opened.
pub(super) fn held(dir: &Path) -> Option<File> {
    lemonfiber_platform::fs::private_directory(dir, true).ok()?;
    let file = lemonfiber_platform::fs::owner_only(
        File::options().create(true).truncate(false).write(true),
    )
    .open(dir.join(LOCK))
    .ok()?;
    for _ in 0..TRIES {
        match file.try_lock() {
            Err(TryLockError::WouldBlock) => std::thread::sleep(BETWEEN),
            locked => return locked.is_ok().then_some(file),
        }
    }
    None
}

/// Whether a copy holds the lock under `dir` now.
fn alive(dir: &Path) -> bool {
    let Ok(file) = File::open(dir.join(LOCK)) else {
        return false;
    };
    matches!(file.try_lock_shared(), Err(TryLockError::WouldBlock))
}

/// The reading under `dir`, where a copy that is still running keeps one this user alone wrote.
pub(super) fn read(dir: &Path) -> Option<Kept> {
    if !alive(dir) {
        return None;
    }
    let at = reading(dir);
    let meta = std::fs::symlink_metadata(&at).ok()?;
    if !lemonfiber_platform::fs::owner_kept(&meta) {
        return None;
    }
    serde_json::from_str(&std::fs::read_to_string(at).ok()?).ok()
}
