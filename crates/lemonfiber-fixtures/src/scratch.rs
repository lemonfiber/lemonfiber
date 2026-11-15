//! A directory of one test's own, removed when the test is done with it.
//!
//! Most tests hold a [`Scratch`] and the directory goes when they finish. Some fixtures
//! hand a path to a context that outlives them, and [`Scratch::kept`] leaves those in
//! place. Every directory a process asks for sits in one directory of that process's
//! own, and the first ask in each process sweeps every other process's directory that
//! has gone [`STALE`].

use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::{Duration, SystemTime};

/// What the name of every process's scratch directory starts with, and all the sweep
/// ever removes.
const PREFIX: &str = "lemonfiber-test-";

/// How long a process's scratch directory goes untouched before the sweep takes it:
/// longer than one test process runs.
const STALE: Duration = Duration::from_secs(30 * 60);

/// A directory unique to one test, removed with everything in it when dropped.
///
/// Named for the file that asked and the name it gave, inside the directory of the
/// process that asked. A file asking for one name twice is given one directory, which
/// is how a test finds again what a fixture it called wrote; two files using one name
/// are given two, so tests running side by side in one process never read each other's
/// files.
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
        SWEPT.call_once(|| sweep(&std::env::temp_dir(), SystemTime::now(), &process()));
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
        let process = process();
        let _ = std::fs::create_dir_all(&process);
        let path = process.join(format!("{asking}-{name}"));
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

/// The directory every scratch directory of this process sits in.
fn process() -> PathBuf {
    std::env::temp_dir().join(format!("{PREFIX}{}", std::process::id()))
}

/// Remove every scratch directory under `root` but `own` that was last touched longer
/// ago than [`STALE`] before `now`.
fn sweep(root: &Path, now: SystemTime, own: &Path) {
    for entry in std::fs::read_dir(root).into_iter().flatten().flatten() {
        let ours = entry.file_name().to_string_lossy().starts_with(PREFIX) && entry.path() != own;
        let stale = entry
            .metadata()
            .and_then(|about| about.modified())
            .is_ok_and(|touched| now.duration_since(touched).is_ok_and(|age| age > STALE));
        if ours && stale {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// The directory `service` mounts from beneath the project at `project` made a link to a
/// directory beside the project, as a container that planted one would leave it, where
/// the platform makes links.
pub fn linked_away(project: &Path, service: &str) {
    let outside = project.with_file_name("outside");
    let at = project.join("config").join(service);
    let _ = std::fs::create_dir_all(&outside);
    let _ = std::fs::create_dir_all(project.join("config"));
    let _ = std::fs::remove_dir_all(&at);
    let _ = lemonfiber_platform::fs::link_directory(&outside, &at);
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
