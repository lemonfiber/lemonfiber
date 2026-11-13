//! A filesystem that keeps what was written to it.
//!
//! The other filesystem fakes answer with what a test seeded and forget writes. A claim
//! on a stack is nothing but a write somebody else can see, so this one keeps them. It
//! leaves the port's own `claim` in place, so the default a fake inherits is the one
//! exercised.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use async_trait::async_trait;
use lemonfiber_ports::filesystem::{
    Fault, FileSystem, FsKind, Identity, Ownership, Storage, StorageFacts,
};

/// A filesystem holding every file written to it, and nothing else.
#[derive(Default)]
pub struct Remembering {
    held: Mutex<HashMap<PathBuf, String>>,
}

impl Remembering {
    /// What is at this path, if anything.
    #[must_use]
    pub fn at(&self, path: &Path) -> Option<String> {
        self.held
            .lock()
            .ok()
            .and_then(|held| held.get(path).cloned())
    }

    /// `contents` put at `path`, as a write would put it.
    pub fn put(&self, path: &Path, contents: &str) {
        if let Ok(mut held) = self.held.lock() {
            held.insert(path.to_path_buf(), contents.to_owned());
        }
    }
}

#[async_trait]
impl FileSystem for Remembering {
    async fn canonicalize(&self, path: &Path) -> Result<PathBuf, Fault> {
        Ok(path.to_path_buf())
    }

    async fn touch(&self, _path: &Path) -> Result<(), Fault> {
        Ok(())
    }

    async fn link(&self, _from: &Path, _to: &Path) -> Result<(), Fault> {
        Ok(())
    }

    async fn identify(&self, _path: &Path) -> Result<Identity, Fault> {
        Err(Fault::new("unused"))
    }

    async fn remove(&self, path: &Path) {
        if let Ok(mut held) = self.held.lock() {
            held.remove(path);
        }
    }

    async fn read(&self, path: &Path) -> Option<String> {
        self.at(path)
    }

    async fn write(&self, path: &Path, contents: &str) {
        self.put(path, contents);
    }

    async fn ownership(&self, _path: &Path) -> Option<Ownership> {
        None
    }
}

#[async_trait]
impl Storage for Remembering {
    async fn describe(&self, _path: &Path) -> StorageFacts {
        StorageFacts {
            point: PathBuf::new(),
            kind: FsKind::Linking("test".to_owned()),
            removable: false,
            available: 0,
            total: 0,
        }
    }
}
