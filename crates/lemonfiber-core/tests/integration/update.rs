//! Moving the stack onto the image versions this build pins.
//!
//! Driven through `dispatch` as every surface reaches it, and from here rather than
//! from a `#[cfg(test)]` module because the confirmed half is `async` end to end — an
//! async path exercised only in-crate has its coverage counted from the copy that
//! never ran.
//!
//! **The engine and the runner are one machine in two halves.** A staged update stops
//! the stack, captures it while nothing can be writing, and then starts one service at
//! a time — so the engine has to say "nothing is running" while the capture is taken
//! and "this one is" immediately afterwards. Two fakes answering independently could
//! not be both, and one answering by how many times it had been asked would be
//! answering a question about this file rather than about the run.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use lemonfiber_core::app::{Command, Ctx, Outcome, Waiting};
use lemonfiber_core::archive::{Archive, Archiving, Fault as ArchiveFault, Reader, Space, Vault};
use lemonfiber_core::backup::{Existing, Item, Manifest as BackupManifest};
use lemonfiber_core::config::paths::Paths;
use lemonfiber_core::config::Settings;
use lemonfiber_core::ports::docker::{Engine, Image};
use lemonfiber_core::ports::process::Runner;
use lemonfiber_core::update::run::Asked;
use lemonfiber_core::update::{Applied, Ending, Reversal};
use lemonfiber_fixtures::files::Files;
use lemonfiber_fixtures::http::Fake;
use lemonfiber_fixtures::pulled::Pulled;
use lemonfiber_fixtures::staged::{Coming, Machine};

/// A version of the curator behind the one the manifest pins.
const CURATOR_BEHIND: &str = "4.0.14";

/// The tag the carried stack pins the curator at.
fn curator_pin() -> String {
    lemonfiber_fixtures::stack::pinned("sonarr")
        .map(|service| service.tag)
        .unwrap_or_default()
}

/// An archive that writes wherever it is told and remembers that it did.
struct Kept {
    /// Whether writing works at all, which is the precondition an update rests on.
    writes: bool,
    /// Every destination it was asked to write, in order.
    wrote: Mutex<Vec<PathBuf>>,
}

impl Kept {
    /// An archive that writes, or one that will not.
    fn writing(writes: bool) -> Arc<Self> {
        Arc::new(Self {
            writes,
            wrote: Mutex::new(Vec::new()),
        })
    }

    /// How many archives it was asked to write.
    fn written(&self) -> usize {
        self.wrote.lock().map_or(0, |wrote| wrote.len())
    }
}

#[async_trait]
impl Archive for Kept {
    async fn space(&self, _dir: &Path, _items: &[Item]) -> Result<Space, ArchiveFault> {
        Ok(Space {
            needed: 0,
            available: 1 << 30,
        })
    }
    async fn write(
        &self,
        dest: &Path,
        _manifest: &BackupManifest,
        _items: &[Item],
    ) -> Result<(), ArchiveFault> {
        if !self.writes {
            return Err(ArchiveFault::new("the disk said no"));
        }
        if let Ok(mut wrote) = self.wrote.lock() {
            wrote.push(dest.to_path_buf());
        }
        Ok(())
    }
    async fn write_files(
        &self,
        _dest: &Path,
        _files: &[(String, String)],
    ) -> Result<(), ArchiveFault> {
        Err(ArchiveFault::new("an update writes no bundle"))
    }
    async fn existing(&self, _dir: &Path) -> Result<Vec<Existing>, ArchiveFault> {
        Ok(Vec::new())
    }
    async fn remove(&self, _dir: &Path, _name: &str) -> Result<(), ArchiveFault> {
        Ok(())
    }
}

/// The other half of the port, so one fake is the one adapter a run holds.
#[async_trait]
impl Reader for Kept {
    async fn read_manifest(&self, _src: &Path) -> Result<BackupManifest, ArchiveFault> {
        Err(ArchiveFault::new("an update never reads an archive back"))
    }
    async fn extract(
        &self,
        _src: &Path,
        _targets: &[(String, PathBuf)],
    ) -> Result<(), ArchiveFault> {
        Err(ArchiveFault::new("an update never reads an archive back"))
    }
}

/// One image this machine has pulled, standing on lemonfiber's own project.
fn pulled(image: &str, tag: &str) -> Image {
    Image {
        tags: vec![format!("{image}:{tag}")],
        bytes: 1,
        projects: vec!["lemonfiber".to_owned()],
    }
}

/// The images a stack standing behind its pins would report.
fn behind(services: &[(&str, &str)]) -> Vec<Image> {
    services
        .iter()
        .map(|(service, tag)| pulled(&format!("lscr.io/linuxserver/{service}"), tag))
        .collect()
}

/// A context over the stack this repository carries, against `machine`.
fn ctx(machine: &Arc<Machine>, images: Vec<Image>, archive: &Arc<Kept>) -> Ctx {
    lemonfiber_testing::a_context()
        .runner(Arc::clone(machine) as Arc<dyn Runner>)
        .engine(Arc::clone(machine) as Arc<dyn Engine>)
        .filesystem(Files::empty())
        .images(Pulled::holding(images))
        .build()
        .with_http(Fake::silent())
        .with_archives(Archiving {
            paths: Paths::rooted(Path::new("/cfg"), Path::new("/data")),
            vault: Arc::clone(archive) as Arc<dyn Vault>,
        })
        .with_patience(Duration::ZERO)
}

/// What was asked of an update: every service, agreed to or not, waiting or not.
fn asking(confirm: bool, wait: Waiting) -> Command {
    Command::Update(Asked {
        service: None,
        confirm,
        wait,
        offer: None,
    })
}

/// The report a dispatched update produced, or nothing where it refused.
fn reported(
    outcome: Result<Outcome, Box<lemonfiber_core::error::Problem>>,
) -> Option<lemonfiber_core::update::run::Report> {
    match outcome {
        Ok(Outcome::Update(report)) => Some(report),
        _ => None,
    }
}

/// What one service came to, by name.
fn came_to(applied: &[Applied], service: &str) -> Option<(Ending, Reversal)> {
    applied
        .iter()
        .find(|one| one.service == service)
        .map(|one| (one.ending, one.reversal))
}

// ── What the run writes down ──────────────────────────────────────────────────

/// The same context, with somewhere to keep a record.
///
/// A whole layout is an environment file and a stack directory, and the contexts
/// above carry neither — so an update run against one has nowhere to write what it
/// did and writes nothing, which is what every case before this one is about. Its stack
/// has no guard to move, so the run is the update's alone.
fn recording(machine: &Arc<Machine>, archive: &Arc<Kept>, name: &str) -> (Ctx, PathBuf) {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("journal-{name}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let stack = dir.join("data").join("stack");
    let _ = std::fs::create_dir_all(&stack);

    let context = lemonfiber_testing::a_context()
        .over(lemonfiber_testing::context::unguarded_stack())
        .runner(Arc::clone(machine) as Arc<dyn Runner>)
        .engine(Arc::clone(machine) as Arc<dyn Engine>)
        .filesystem(Files::empty())
        .settings(Settings {
            env_file: Some(dir.join(".env")),
            stack_dir: Some(stack),
            ..Settings::default()
        })
        .build()
        .with_images(Pulled::holding(behind(&[("sonarr", CURATOR_BEHIND)])))
        .with_http(Fake::silent())
        .with_archives(Archiving {
            paths: Paths::rooted(Path::new("/cfg"), Path::new("/data")),
            vault: Arc::clone(archive) as Arc<dyn Vault>,
        })
        .with_patience(Duration::ZERO);

    (context, dir.join("journal.jsonl"))
}

// ── What is still coming down ─────────────────────────────────────────────────

mod capturing;
mod checking;
mod finishing;
mod restarting;
