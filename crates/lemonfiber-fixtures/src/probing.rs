//! A filesystem probed for whether a location can be written to and hardlinked.
//!
//! Every answer a probe reads is a field a test sets: whether a path resolves, whether a
//! probe file can be made, whether a link takes, what the two names identify as, and what
//! the volume is. Identity is asked of the original and of the link, told apart by the
//! name asked about, so a test can make the two disagree.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lemonfiber_ports::filesystem::{
    Fault, FileSystem, FsKind, Identity, Ownership, Storage, StorageFacts,
};

/// What a resolution that has not found the path yet says.
const MISSING: &str = "no such file or directory";

/// What a link that has not taken yet says.
const REFUSED: &str = "operation not permitted";

/// The file number every name identifies as unless a test says otherwise.
const FILE: u64 = 7;

/// Free space on a healthy volume, enough that a probe not about space never trips a floor.
pub const AMPLE: u64 = 500 * 1024 * 1024 * 1024;

/// The size of a healthy volume, a terabyte.
pub const CAPACITY: u64 = 1024 * 1024 * 1024 * 1024;

/// A filesystem whose every answer to a probe the test scripts.
pub struct Probing {
    /// What resolving any path answers: this path, the path asked about where none, or
    /// the fault.
    pub resolves: Result<Option<PathBuf>, Fault>,
    /// How many resolutions fail as missing before they answer as `resolves` does, so a
    /// leaf can be absent while a parent of it is found.
    pub missing_leaves: AtomicUsize,
    /// What making a probe file answers.
    pub writes: Result<(), Fault>,
    /// What a link answers once every scripted refusal has been given.
    pub links: Result<(), Fault>,
    /// How many links are refused before they answer as `links` does, so one location can
    /// refuse to link and the next accept it.
    pub failing_links: AtomicUsize,
    /// What the original name identifies as.
    pub original: Result<Identity, Fault>,
    /// What the linked name, ending `.link`, identifies as.
    pub confirmed: Result<Identity, Fault>,
    /// What the volume is.
    pub facts: StorageFacts,
    /// What every read answers.
    pub remembered: Option<String>,
    /// Who owns every path.
    pub owner: Option<Ownership>,
    /// Everything written, in order.
    pub recorded: Arc<Mutex<Vec<String>>>,
}

impl Probing {
    /// A healthy local volume: resolves to `/data`, writes, links, and reports the two
    /// names as one file on a filesystem that links, with [`AMPLE`] free of [`CAPACITY`].
    #[must_use]
    pub fn healthy() -> Self {
        Self {
            resolves: Ok(Some(PathBuf::from("/data"))),
            original: Ok(Identity {
                file: FILE,
                links: 1,
            }),
            facts: StorageFacts {
                point: PathBuf::from("/"),
                kind: FsKind::Linking("apfs".to_owned()),
                removable: false,
                available: AMPLE,
                total: CAPACITY,
            },
            ..Self::linking()
        }
    }

    /// A volume that links wherever it is asked: every path resolves as itself, and both
    /// names identify as one file with two links.
    #[must_use]
    pub fn linking() -> Self {
        Self {
            resolves: Ok(None),
            missing_leaves: AtomicUsize::new(0),
            writes: Ok(()),
            links: Ok(()),
            failing_links: AtomicUsize::new(0),
            original: Ok(Identity {
                file: FILE,
                links: 2,
            }),
            confirmed: Ok(Identity {
                file: FILE,
                links: 2,
            }),
            facts: StorageFacts {
                point: PathBuf::new(),
                kind: FsKind::Linking("apfs".to_owned()),
                removable: false,
                available: 0,
                total: 0,
            },
            remembered: None,
            owner: None,
            recorded: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

/// Take one from a countdown, answering whether there was one to take.
fn counted_down(remaining: &AtomicUsize) -> bool {
    remaining
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |left| {
            left.checked_sub(1)
        })
        .is_ok()
}

#[async_trait]
impl FileSystem for Probing {
    async fn canonicalize(&self, path: &Path) -> Result<PathBuf, Fault> {
        let resolved = self.resolves.clone()?;
        if counted_down(&self.missing_leaves) {
            return Err(Fault::new(MISSING));
        }
        Ok(resolved.unwrap_or_else(|| path.to_owned()))
    }

    async fn touch(&self, _path: &Path) -> Result<(), Fault> {
        self.writes.clone()
    }

    async fn link(&self, _from: &Path, _to: &Path) -> Result<(), Fault> {
        if counted_down(&self.failing_links) {
            return Err(Fault::new(REFUSED));
        }
        self.links.clone()
    }

    async fn identify(&self, path: &Path) -> Result<Identity, Fault> {
        if path.to_string_lossy().ends_with(".link") {
            self.confirmed.clone()
        } else {
            self.original.clone()
        }
    }

    async fn remove(&self, _path: &Path) {}

    async fn read(&self, _path: &Path) -> Option<String> {
        self.remembered.clone()
    }

    async fn write(&self, _path: &Path, contents: &str) {
        crate::noted(&self.recorded, contents.to_owned());
    }

    async fn ownership(&self, _path: &Path) -> Option<Ownership> {
        self.owner
    }
}

#[async_trait]
impl Storage for Probing {
    async fn describe(&self, _path: &Path) -> StorageFacts {
        self.facts.clone()
    }
}
