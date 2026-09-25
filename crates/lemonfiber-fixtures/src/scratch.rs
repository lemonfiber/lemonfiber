//! A directory of one test's own, removed when the test is done with it.
//!
//! Most tests hold a [`Scratch`] and the directory goes when they finish. Some fixtures
//! hand a path to a context that outlives them, and [`Scratch::kept`] leaves those in
//! place; what a run leaves is swept by the next run to ask for a directory once it is
//! a day old, so nothing a test writes stays on the machine for good.

use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::{Duration, SystemTime};

/// What every scratch directory's name starts with, and all the sweep ever removes.
const PREFIX: &str = "lemonfiber-test-";

/// How old a directory is before the sweep takes it: long enough that no run still
/// going could be using it.
const STALE: Duration = Duration::from_secs(24 * 60 * 60);

/// A directory unique to one test, removed with everything in it when dropped.
///
/// Named for the process, the file that asked and the name it gave. A file asking
/// for one name twice is given one directory, which is how a test finds again what a
/// fixture it called wrote; two files using one name are given two, so tests running
/// side by side in one process never read each other's files.
///
/// It stands for a path inside that directory where a test is about one file in it,
/// and the whole directory still goes when it is dropped.
#[derive(Debug)]
pub struct Scratch {
    root: PathBuf,
    at: PathBuf,
}

impl Scratch {
    /// A new, empty directory for the test `name`.
    #[must_use]
    #[track_caller]
    pub fn new(name: &str) -> Self {
        let scratch = Self::unmade(name);
        let _ = std::fs::create_dir_all(&scratch.root);
        scratch
    }

    /// A path of the test `name`'s own that nothing exists at yet, for the tests about
    /// what creates it.
    #[must_use]
    #[track_caller]
    pub fn unmade(name: &str) -> Self {
        let scratch = Self::named(name);
        let _ = std::fs::remove_dir_all(&scratch.root);
        scratch
    }

    /// The test `name`'s directory as it stands, emptied of nothing: for a test
    /// finding again what a fixture it called has already written there.
    #[must_use]
    #[track_caller]
    pub fn named(name: &str) -> Self {
        static SWEPT: Once = Once::new();
        SWEPT.call_once(|| sweep(&std::env::temp_dir(), SystemTime::now()));
        let asking: String = std::panic::Location::caller()
            .file()
            .chars()
            .map(|letter| {
                if letter.is_ascii_alphanumeric() {
                    letter
                } else {
                    '-'
                }
            })
            .collect();
        let path =
            std::env::temp_dir().join(format!("{PREFIX}{}-{asking}-{name}", std::process::id()));
        Self {
            at: path.clone(),
            root: path,
        }
    }

    /// The same directory, standing for `relative` inside it.
    #[must_use]
    pub fn within(mut self, relative: &str) -> Self {
        // Taken rather than cloned: `self` is dropped on the way out, and a drop that
        // still held the root would remove the directory being handed back.
        let root = std::mem::take(&mut self.root);
        Self {
            at: root.join(relative),
            root,
        }
    }

    /// The path, left in place when this is dropped: for a fixture handing it to a
    /// context that outlives the fixture. The sweep removes it once it is stale.
    #[must_use]
    pub fn kept(mut self) -> PathBuf {
        self.root = PathBuf::new();
        std::mem::take(&mut self.at)
    }

    /// The path it stands for.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.at
    }
}

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.at
    }
}

impl AsRef<Path> for Scratch {
    fn as_ref(&self) -> &Path {
        &self.at
    }
}

/// Remove every scratch directory under `root` that was last touched longer ago than
/// [`STALE`] before `now`.
fn sweep(root: &Path, now: SystemTime) {
    for entry in std::fs::read_dir(root).into_iter().flatten().flatten() {
        let ours = entry.file_name().to_string_lossy().starts_with(PREFIX);
        let stale = entry
            .metadata()
            .and_then(|about| about.modified())
            .is_ok_and(|touched| now.duration_since(touched).is_ok_and(|age| age > STALE));
        if ours && stale {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if !self.root.as_os_str().is_empty() {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
}

#[cfg(test)]
mod tests;
