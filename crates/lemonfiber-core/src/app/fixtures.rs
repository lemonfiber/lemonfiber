//! Stand-in adapters the app layer's own tests are written against.
//!
//! One fake per port, shared by every module that drives it. Two fakes for one port drift
//! apart — they answer the same call differently, and a test then proves something about
//! its own stand-in rather than about the code under it.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::archive::{Archive, Archiving, Fault, Reader, Space};
use crate::backup::{self, Existing, Item, Manifest, Scope, SCHEMA};
use crate::config::paths::Paths;
use crate::test_support::a_context;

/// More room than any writer asks to keep free, so a test that is not about room does not
/// have to know what the writer it drives keeps in reserve.
const ROOMY: u64 = 512 * 1024 * 1024;

/// The layout every test about an archive is written against.
///
/// One layout rather than one per module: what a capture writes and what a restore
/// puts back are the same directories, and two spellings of them would let a test
/// pass while proving something about a tree the other half never touches.
pub(crate) fn paths() -> Paths {
    Paths::rooted(Path::new("/cfg"), Path::new("/data"))
}

/// An archive that answers each operation from what the test scripted, and records what it
/// was asked to write, remove and unpack.
///
/// Both halves of the port, because a run holds one adapter that does both: a capture
/// writes archives and a restore reads them, and a fake per half would be two stand-ins
/// that could answer the same question differently.
pub(crate) struct FakeArchive {
    pub(crate) space: Result<Space, Fault>,
    pub(crate) write: Result<(), Fault>,
    pub(crate) existing: Result<Vec<Existing>, Fault>,
    pub(crate) remove: Result<(), Fault>,
    pub(crate) manifest: Result<Manifest, Fault>,
    pub(crate) extract: Result<(), Fault>,
    pub(crate) written: Mutex<Vec<PathBuf>>,
    pub(crate) removed: Mutex<Vec<String>>,
    pub(crate) extracted: Mutex<Vec<PathBuf>>,
}

impl FakeArchive {
    /// An archive with ample room where every operation succeeds and no older backups
    /// exist to prune, holding a whole-stack manifest this build can restore.
    pub(crate) fn roomy() -> Self {
        Self {
            space: Ok(Space {
                needed: 10,
                available: ROOMY,
            }),
            write: Ok(()),
            existing: Ok(Vec::new()),
            remove: Ok(()),
            manifest: Ok(manifest_of(CURRENT, SCHEMA)),
            extract: Ok(()),
            written: Mutex::new(Vec::new()),
            removed: Mutex::new(Vec::new()),
            extracted: Mutex::new(Vec::new()),
        }
    }

    /// The same archive, holding a whole-stack manifest written by `version` in
    /// format `schema` — the two things a restore decides on before it overwrites.
    pub(crate) fn holding(version: &str, schema: u32) -> Self {
        Self {
            manifest: Ok(manifest_of(version, schema)),
            ..Self::roomy()
        }
    }

    /// The same archive, keeping the backups named — each with the moment it was
    /// taken, written the way the adapter writes one.
    pub(crate) fn keeping_backups(kept: &[(&str, &str)]) -> Self {
        Self {
            existing: Ok(kept
                .iter()
                .map(|(name, taken)| Existing {
                    name: (*name).to_owned(),
                    created_at: (*taken).to_owned(),
                })
                .collect()),
            ..Self::roomy()
        }
    }

    /// The same archive, whose directory will not be read at all.
    pub(crate) fn unlistable() -> Self {
        Self {
            existing: Err(Fault::new("permission denied")),
            ..Self::roomy()
        }
    }

    /// What it was asked to unpack, in the order it was asked.
    pub(crate) fn extractions(&self) -> Vec<PathBuf> {
        self.extracted.lock().map(|e| e.clone()).unwrap_or_default()
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

/// The version a test restores against unless it is about a version gap.
pub(crate) const CURRENT: &str = "0.3.0";

/// A whole-stack manifest taken against `/srv/media`, as an archive carries one.
fn manifest_of(version: &str, schema: u32) -> Manifest {
    let plan = backup::plan(&paths(), &Scope::WholeStack);
    let mut manifest = Manifest::describe(&plan, version, "t", "/srv/media");
    manifest.schema = schema;
    manifest
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

#[async_trait]
impl Reader for FakeArchive {
    async fn read_manifest(&self, _src: &Path) -> Result<Manifest, Fault> {
        self.manifest.clone()
    }
    async fn extract(&self, src: &Path, _targets: &[(String, PathBuf)]) -> Result<(), Fault> {
        if let Ok(mut extracted) = self.extracted.lock() {
            extracted.push(src.to_path_buf());
        }
        self.extract.clone()
    }
}

/// A context that keeps its archives in the shared layout, through `vault`.
///
/// The pair a capture and a restore both need, handed over the way a surface hands
/// it over — so what a test drives is the run holding an adapter, not a function
/// taking one.
pub(crate) fn keeping(ctx: crate::app::Ctx, vault: &Arc<FakeArchive>) -> crate::app::Ctx {
    ctx.keeping(Archiving {
        paths: paths(),
        vault: Arc::clone(vault) as Arc<dyn crate::archive::Vault>,
    })
}

/// A scratch directory unique to one test, emptied first.
///
/// Unique per test and per process because these records land beside the environment file,
/// and two tests sharing one directory would each be reading what the other wrote.
pub(crate) fn scratch(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("lemonfiber-app-{}-{name}", std::process::id()))
}

/// A context whose environment file is in that scratch directory, so every record a test
/// drives lands somewhere of its own.
///
/// Shared rather than copied into each module that needs one: three copies of a context
/// builder are three places for a test to be set up subtly differently from the code it is
/// meant to be proving.
pub(crate) fn ctx_at(name: &str) -> crate::app::Ctx {
    let dir = scratch(name);
    let _ = std::fs::remove_dir_all(&dir);
    let settings = crate::config::Settings {
        env_file: Some(dir.join(".env")),
        ..crate::config::Settings::default()
    };
    a_context()
        .runner(std::sync::Arc::new(crate::test_support::Scripted(Ok(
            crate::test_support::spoke(""),
        ))))
        .settings(settings)
        .build()
}
