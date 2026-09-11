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
//! Reading is meaningful, and so is the mode a path is guarded by — the second
//! because a check about permissions has nothing else to look at. Every other
//! capability answers as unused, because a path that reached for one would be a path
//! this fake was never meant to stand in for, and a plausible answer there would hide
//! it. A filesystem no test gave modes to reports none, which is what a file that is
//! not there looks like.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_ports::filesystem::{
    Fault, FileSystem, FsKind, Identity, Ownership, Storage, StorageFacts,
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
    /// The permission bits reported for a path ending in each fragment.
    ///
    /// A second field rather than another [`Held`], because what a file holds and how
    /// it is guarded are two independent questions: a test about one should not have
    /// to answer the other. Empty everywhere but the tests whose subject is
    /// permissions, and a path with no entry reports no ownership at all — which is
    /// what a file that is not there looks like.
    modes: Vec<(&'static str, u32)>,
}

impl Files {
    /// One file, handed to any reader.
    #[must_use]
    pub fn anywhere(text: impl Into<String>) -> Arc<Self> {
        Self::new(Held::Anywhere(Some(text.into())))
    }

    /// Nothing at all: no service has written its configuration yet.
    #[must_use]
    pub fn empty() -> Arc<Self> {
        Self::new(Held::Anywhere(None))
    }

    /// Text at each of these exact paths.
    #[must_use]
    pub fn at(files: Vec<(PathBuf, &str)>) -> Arc<Self> {
        Self::new(Held::At(
            files
                .into_iter()
                .map(|(path, text)| (path, text.to_owned()))
                .collect(),
        ))
    }

    /// Text for each path ending in one of these fragments.
    #[must_use]
    pub fn ending(files: Vec<(&'static str, &str)>) -> Arc<Self> {
        Self::new(Held::Ending(
            files
                .into_iter()
                .map(|(ending, text)| (ending, text.to_owned()))
                .collect(),
        ))
    }

    /// A filesystem reporting these permission bits for paths ending in each
    /// fragment, and holding no file anywhere.
    #[must_use]
    pub fn owning(modes: Vec<(&'static str, u32)>) -> Arc<Self> {
        Arc::new(Self {
            held: Held::Anywhere(None),
            modes,
        })
    }

    fn new(held: Held) -> Arc<Self> {
        Arc::new(Self {
            held,
            modes: Vec::new(),
        })
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
        held_at(&self.held, path)
    }

    async fn write(&self, _path: &Path, _contents: &str) {}

    async fn ownership(&self, path: &Path) -> Option<Ownership> {
        let path = path.to_string_lossy().replace('\\', "/");
        self.modes
            .iter()
            .find(|(ending, _)| path.ends_with(ending))
            .map(|(_, mode)| Ownership {
                uid: 0,
                gid: 0,
                mode: *mode,
            })
    }
}

#[async_trait]
impl Storage for Files {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A filesystem given modes reports them for the paths it was given and for no
    /// others, so a check about permissions can tell a widened file from an absent one.
    #[tokio::test]
    async fn a_mode_is_reported_for_a_path_it_was_given_and_for_no_other() {
        let files = Files::owning(vec![(".env", 0o644)]);

        let found = files
            .ownership(Path::new("/somewhere/lemonfiber/.env"))
            .await;
        assert_eq!(found.map(|one| one.mode), Some(0o644));
        assert!(files
            .ownership(Path::new("/somewhere/lemonfiber/admission.json"))
            .await
            .is_none());
    }

    /// The stub half of the contract, which the doc above states and nothing checked.
    ///
    /// A test that reaches one of these has reached for a capability this fake was
    /// never meant to stand in for, so each must refuse rather than answer plausibly.
    /// Asserting that here is also what keeps them reachable: nothing else calls them,
    /// and an unreached line is one the gate counts against a file it cannot see is
    /// deliberate. A filesystem given no modes reports none for the same reason.
    #[tokio::test]
    async fn every_capability_but_reading_answers_as_unused() {
        let files = Files::anywhere("held");
        let path = Path::new("/srv/config.xml");

        assert_eq!(
            files.canonicalize(path).await.ok().as_deref(),
            Some(path),
            "a path canonicalises to itself: there is no filesystem to resolve it against"
        );
        assert!(files.touch(path).await.is_err());
        assert!(files.link(path, path).await.is_err());
        assert!(files.identify(path).await.is_err());
        assert!(files.ownership(path).await.is_none());

        // Neither answers anything, and neither may panic: a test that writes through
        // this fake is one whose subject is what it read back, not what it wrote.
        files.remove(path).await;
        files.write(path, "ignored").await;
        assert_eq!(
            files.read(path).await.as_deref(),
            Some("held"),
            "a write changes nothing: what is held is what the test scripted"
        );

        let facts = files.describe(path).await;
        assert!(
            matches!(facts.kind, FsKind::Linking(_)) && !facts.removable,
            "a filesystem that links and is not removable, so the storage probe reads \
             as the ordinary case rather than a warning a test did not ask for"
        );
    }
}

/// What this fixture holds at a path, by whichever way it was told to hold it.
fn held_at(held: &Held, path: &Path) -> Option<String> {
    match held {
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
