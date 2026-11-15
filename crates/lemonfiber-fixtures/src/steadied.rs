//! A filesystem answering as another does, with what differs from one machine to the next
//! held still: who owns every path, and the filesystem every path is on.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_ports::filesystem::{Fault, FileSystem, Identity, Ownership, Storage, StorageFacts};

/// A filesystem answering as the one it is over, every path owned by one id and on one
/// filesystem.
pub struct Steadied {
    over: Arc<dyn FileSystem>,
    owner: u32,
    facts: StorageFacts,
}

impl Steadied {
    /// `over`, every path owned by `owner` as both user and group, and on the filesystem
    /// `facts` describes.
    #[must_use]
    pub fn over(over: Arc<dyn FileSystem>, owner: u32, facts: StorageFacts) -> Self {
        Self { over, owner, facts }
    }
}

#[async_trait]
impl Storage for Steadied {
    async fn describe(&self, _path: &Path) -> StorageFacts {
        self.facts.clone()
    }
}

#[async_trait]
impl FileSystem for Steadied {
    async fn canonicalize(&self, path: &Path) -> Result<PathBuf, Fault> {
        self.over.canonicalize(path).await
    }

    async fn touch(&self, path: &Path) -> Result<(), Fault> {
        self.over.touch(path).await
    }

    async fn link(&self, from: &Path, to: &Path) -> Result<(), Fault> {
        self.over.link(from, to).await
    }

    async fn identify(&self, path: &Path) -> Result<Identity, Fault> {
        self.over.identify(path).await
    }

    async fn remove(&self, path: &Path) {
        self.over.remove(path).await;
    }

    async fn read(&self, path: &Path) -> Option<String> {
        self.over.read(path).await
    }

    async fn write(&self, path: &Path, contents: &str) {
        self.over.write(path, contents).await;
    }

    async fn ownership(&self, path: &Path) -> Option<Ownership> {
        let owned = self.over.ownership(path).await?;
        Some(Ownership {
            uid: self.owner,
            gid: self.owner,
            ..owned
        })
    }
}
