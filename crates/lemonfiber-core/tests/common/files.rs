//! The filesystem every integration test reads a service's own configuration through.
//!
//! Four of these test crates each declared their own, and they were the same eight
//! stub methods around one meaningful `read` — differing only in how a path becomes
//! text. Four copies is four places for the semantics to drift, and the fifth was
//! about to be written when this appeared.
//!
//! One filesystem with three ways to script what it holds. Nothing is held anywhere
//! a test did not put it: a read of an unscripted path answers with nothing, which is
//! what a service that has not written its key yet looks like, so a path a test did
//! not mean to reach shows up as the skip it should be rather than as a fabricated
//! file.
//!
//! Only `read` is meaningful. Every other capability answers as unused, because a
//! path that reached for one would be a path this fake was never meant to stand in
//! for — and a plausible answer there would hide it.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_core::ports::filesystem::{
    Fault, FileSystem, FsKind, Identity, Ownership, StorageFacts,
};

/// What the filesystem holds, and how a path finds it.
pub enum Held {
    /// The same text at every path — a filesystem with one file it hands to any
    /// reader, for a test that does not care where the reader looked.
    Anywhere(Option<String>),
    /// The text placed at each path exactly, and nothing anywhere else.
    At(Vec<(PathBuf, String)>),
    /// The text for the first path that ends with the fragment given — for a config
    /// whose full path is assembled from a project root the test would rather not
    /// spell out.
    Ending(Vec<(&'static str, String)>),
}

/// A filesystem holding what a test placed in it.
pub struct Files {
    held: Held,
}

impl Files {
    /// One file, handed to any reader.
    pub fn anywhere(text: impl Into<String>) -> Arc<Self> {
        Self::new(Held::Anywhere(Some(text.into())))
    }

    /// Nothing at all: no service has written its configuration yet.
    pub fn empty() -> Arc<Self> {
        Self::new(Held::Anywhere(None))
    }

    /// Text at each of these exact paths.
    pub fn at(files: Vec<(PathBuf, &str)>) -> Arc<Self> {
        Self::new(Held::At(
            files
                .into_iter()
                .map(|(path, text)| (path, text.to_owned()))
                .collect(),
        ))
    }

    /// Text for each path ending in one of these fragments.
    pub fn ending(files: Vec<(&'static str, &str)>) -> Arc<Self> {
        Self::new(Held::Ending(
            files
                .into_iter()
                .map(|(ending, text)| (ending, text.to_owned()))
                .collect(),
        ))
    }

    fn new(held: Held) -> Arc<Self> {
        Arc::new(Self { held })
    }
}

#[async_trait]
impl FileSystem for Files {
    async fn canonicalize(&self, path: &Path) -> Result<PathBuf, Fault> {
        Ok(path.to_path_buf())
    }

    async fn touch(&self, _path: &Path) -> Result<(), Fault> {
        Err(Fault::new("unused"))
    }

    async fn link(&self, _from: &Path, _to: &Path) -> Result<(), Fault> {
        Err(Fault::new("unused"))
    }

    async fn identify(&self, _path: &Path) -> Result<Identity, Fault> {
        Err(Fault::new("unused"))
    }

    async fn remove(&self, _path: &Path) {}

    async fn read(&self, path: &Path) -> Option<String> {
        match &self.held {
            Held::Anywhere(text) => text.clone(),
            Held::At(files) => files
                .iter()
                .find(|(at, _)| at == path)
                .map(|(_, text)| text.clone()),
            Held::Ending(files) => {
                // Separators normalised, so a fragment spelled the way the stack spells it
                // matches on a host that spells paths the other way round.
                let path = path.to_string_lossy().replace('\\', "/");
                files
                    .iter()
                    .find(|(ending, _)| path.ends_with(ending))
                    .map(|(_, text)| text.clone())
            }
        }
    }

    async fn write(&self, _path: &Path, _contents: &str) {}

    async fn ownership(&self, _path: &Path) -> Option<Ownership> {
        None
    }

    async fn describe(&self, _path: &Path) -> StorageFacts {
        StorageFacts {
            kind: FsKind::Linking("test".to_owned()),
            removable: false,
            available: 0,
            total: 0,
        }
    }
}
