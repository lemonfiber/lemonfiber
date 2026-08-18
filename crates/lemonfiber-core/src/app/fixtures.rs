//! Stand-in adapters the app layer's own tests are written against.
//!
//! One fake per port, shared by every module that drives it. Two fakes for one port drift
//! apart — they answer the same call differently, and a test then proves something about
//! its own stand-in rather than about the code under it.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use async_trait::async_trait;

use crate::backup::{Existing, Item, Manifest};
use crate::ports::archive::{Archive, Fault, Space};

/// More room than any writer asks to keep free, so a test that is not about room does not
/// have to know what the writer it drives keeps in reserve.
const ROOMY: u64 = 512 * 1024 * 1024;

/// An archive that answers each operation from what the test scripted, and records what it
/// was asked to write and remove.
pub(crate) struct FakeArchive {
    pub(crate) space: Result<Space, Fault>,
    pub(crate) write: Result<(), Fault>,
    pub(crate) existing: Result<Vec<Existing>, Fault>,
    pub(crate) remove: Result<(), Fault>,
    pub(crate) written: Mutex<Vec<PathBuf>>,
    pub(crate) removed: Mutex<Vec<String>>,
}

impl FakeArchive {
    /// An archive with ample room where every operation succeeds and no older backups
    /// exist to prune.
    pub(crate) fn roomy() -> Self {
        Self {
            space: Ok(Space {
                needed: 10,
                available: ROOMY,
            }),
            write: Ok(()),
            existing: Ok(Vec::new()),
            remove: Ok(()),
            written: Mutex::new(Vec::new()),
            removed: Mutex::new(Vec::new()),
        }
    }

    /// Where it was asked to write, in the order it was asked.
    pub(crate) fn writes(&self) -> Vec<PathBuf> {
        self.written.lock().map(|w| w.clone()).unwrap_or_default()
    }

    /// What it was asked to prune.
    pub(crate) fn removes(&self) -> Vec<String> {
        self.removed.lock().map(|r| r.clone()).unwrap_or_default()
    }

    /// Both writers record the same way and answer the same script: what a test asks of
    /// this fake is where it was told to write, never which of the two ports asked.
    fn record(&self, dest: &Path) -> Result<(), Fault> {
        if let Ok(mut written) = self.written.lock() {
            written.push(dest.to_path_buf());
        }
        self.write.clone()
    }
}

#[async_trait]
impl Archive for FakeArchive {
    async fn space(&self, _dir: &Path, _items: &[Item]) -> Result<Space, Fault> {
        self.space.clone()
    }
    async fn write(&self, dest: &Path, _manifest: &Manifest, _items: &[Item]) -> Result<(), Fault> {
        self.record(dest)
    }
    async fn write_files(&self, dest: &Path, _files: &[(String, String)]) -> Result<(), Fault> {
        self.record(dest)
    }
    async fn existing(&self, _dir: &Path) -> Result<Vec<Existing>, Fault> {
        self.existing.clone()
    }
    async fn remove(&self, _dir: &Path, name: &str) -> Result<(), Fault> {
        if let Ok(mut removed) = self.removed.lock() {
            removed.push(name.to_owned());
        }
        self.remove.clone()
    }
}
