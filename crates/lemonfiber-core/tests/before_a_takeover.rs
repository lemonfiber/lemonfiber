//! The capture taken before lemonfiber takes over a setup that was already here.
//!
//! Beside `what_is_already_here.rs` rather than inside it because it is a different
//! seam: that file is about the survey and what may be *done* about what it found,
//! and this is about the one thing that must happen first. They share a machine and
//! almost nothing else — what is asserted here is which tree was copied, whether it
//! was safe to copy it, and that nothing was written until it had been.
//!
//! The capture is driven through the dispatcher rather than called directly, because
//! the ordering is the point: an adoption that wrote its answer and then failed to
//! capture would leave an operator believing lemonfiber manages a stack whose
//! configuration was never protected.

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use common::stack::project;
use lemonfiber_core::app::{dispatch, Command, Ctx, MigrateAction, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::migration::mode::Mode;
use lemonfiber_core::model::AdoptReport;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::docker::{Health, Lifecycle};
use lemonfiber_core::ports::filesystem::{FsKind, StorageFacts};
use lemonfiber_core::reconfigure::Stance;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::pulled::Pulled;
use lemonfiber_fixtures::support::{spoke, Reporting, Scripted, SeedFs};

/// A machine whose existing setup holds one service lemonfiber knows.
fn over(engine: Reporting, images: Arc<Pulled>) -> Ctx {
    Ctx::new(
        Arc::new(Scripted(Ok(spoke("")))),
        Arc::new(engine),
        lemonfiber_fixtures::ports::Stopped::today(),
        lemonfiber_ports::seams::Seams {
            filesystem: Arc::new(SeedFs::keyed(None, None).with_facts(StorageFacts {
                point: PathBuf::from("/srv/media"),
                kind: FsKind::Linking("apfs".to_owned()),
                removable: false,
                available: 100,
                total: 1_000,
            })),
            ..lemonfiber_adapters::live()
        },
        Source::External(project()),
        Settings {
            project: "lemonfiber".to_owned(),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_images(images)
}

/// An engine holding one service under a project that is not lemonfiber's.
fn theirs(lifecycle: Lifecycle, mounts: &[&str]) -> Reporting {
    Reporting::holding(&["sonarr"], lifecycle, Health::None)
        .belonging_to("media")
        .mounting(&mounts.iter().map(PathBuf::from).collect::<Vec<_>>())
}

/// The image that makes the survey see a setup worth taking over.
fn images() -> Arc<Pulled> {
    Pulled::holding(vec![Pulled::image(
        "lscr.io/linuxserver/sonarr:4.0.0",
        400,
        &["media"],
    )])
}

/// Somewhere to write the answer, emptied first and unique to one test.
fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("lemonfiber-takeover-{}-{name}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let env = dir.join(".env");
    let _ = std::fs::remove_file(&env);
    env
}

/// Somewhere for the capture an adoption takes first, remembering what it was handed.
///
/// It records the sources rather than the destination, because what is being proved
/// here is *which tree* was captured — and a fake that only remembered that it wrote
/// would answer a question nobody is asking.
#[derive(Default)]
struct Captured(std::sync::Mutex<Vec<PathBuf>>);

impl Captured {
    /// Every source a capture was asked to read, in the order it was asked.
    fn sources(&self) -> Vec<PathBuf> {
        self.0.lock().map(|seen| seen.clone()).unwrap_or_default()
    }
}

#[async_trait::async_trait]
impl lemonfiber_core::archive::Archive for Captured {
    async fn space(
        &self,
        _dir: &Path,
        _items: &[lemonfiber_core::backup::Item],
    ) -> Result<lemonfiber_core::archive::Space, lemonfiber_core::archive::Fault> {
        Ok(lemonfiber_core::archive::Space {
            needed: 0,
            available: 1 << 30,
        })
    }
    async fn write(
        &self,
        _dest: &Path,
        _manifest: &lemonfiber_core::backup::Manifest,
        items: &[lemonfiber_core::backup::Item],
    ) -> Result<(), lemonfiber_core::archive::Fault> {
        if let Ok(mut seen) = self.0.lock() {
            seen.extend(items.iter().map(|item| item.source.clone()));
        }
        Ok(())
    }
    async fn write_files(
        &self,
        _dest: &Path,
        _files: &[(String, String)],
    ) -> Result<(), lemonfiber_core::archive::Fault> {
        Err(lemonfiber_core::archive::Fault::new(
            "an adoption writes no bundle",
        ))
    }
    async fn existing(
        &self,
        _dir: &Path,
    ) -> Result<Vec<lemonfiber_core::backup::Existing>, lemonfiber_core::archive::Fault> {
        Ok(Vec::new())
    }
    async fn remove(
        &self,
        _dir: &Path,
        _name: &str,
    ) -> Result<(), lemonfiber_core::archive::Fault> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl lemonfiber_core::archive::Reader for Captured {
    async fn read_manifest(
        &self,
        _src: &Path,
    ) -> Result<lemonfiber_core::backup::Manifest, lemonfiber_core::archive::Fault> {
        Err(lemonfiber_core::archive::Fault::new(
            "an adoption never reads an archive back",
        ))
    }
    async fn extract(
        &self,
        _src: &Path,
        _targets: &[(String, PathBuf)],
    ) -> Result<(), lemonfiber_core::archive::Fault> {
        Err(lemonfiber_core::archive::Fault::new(
            "an adoption never reads an archive back",
        ))
    }
}

/// The same machine, with somewhere to keep what an adoption captures first.
fn keeping(ctx: Ctx, vault: &Arc<Captured>) -> Ctx {
    ctx.keeping(lemonfiber_core::archive::Archiving {
        paths: lemonfiber_core::config::paths::Paths::rooted(Path::new("/cfg"), Path::new("/data")),
        vault: Arc::clone(vault) as Arc<dyn lemonfiber_core::archive::Vault>,
    })
}

/// A machine ready to be taken over, keeping whatever the adoption captures.
fn machine(lifecycle: Lifecycle, mounts: &[&str], name: &str) -> (Ctx, Arc<Captured>, PathBuf) {
    let env = scratch(name);
    let mut ctx = over(theirs(lifecycle, mounts), images());
    ctx.settings.env_file = Some(env.clone());
    let vault = Arc::new(Captured::default());
    (keeping(ctx, &vault), Arc::clone(&vault), env)
}

/// What adopting answered, or nothing where it refused to answer at all.
async fn adopting(ctx: &Ctx, confirmed: bool) -> Option<AdoptReport> {
    match dispatch(
        Command::Migrate(MigrateAction::Act {
            mode: Mode::Adopt,
            confirmed,
        }),
        ctx,
    )
    .await
    {
        Ok(Outcome::Adoption(report)) => Some(report),
        _ => None,
    }
}

/// The capture covers the setup being taken over, at the host paths the survey
/// reported — not lemonfiber's own layout, which holds nothing worth protecting
/// until the takeover has happened.
#[tokio::test]
async fn adopting_captures_the_existing_setups_own_paths_before_it_writes_anything() {
    let (ctx, vault, _) = machine(Lifecycle::Exited, &["/srv/their-media"], "captured");

    let found = adopting(&ctx, true).await;
    assert_eq!(
        found.as_ref().map(|read| read.stance),
        Some(Stance::Applied),
        "{found:?}"
    );
    assert_eq!(
        vault.sources(),
        vec![PathBuf::from("/srv/their-media")],
        "the capture covered lemonfiber's own tree rather than the one being taken over"
    );
    assert!(
        found.and_then(|read| read.backed_up).is_some(),
        "an operator told a backup was taken is owed the path to it"
    );
}

/// A setup still running cannot be captured, so it cannot be adopted either.
///
/// The refusal is what proves the project being asked about is the existing setup's:
/// lemonfiber's own does not exist yet, and an engine asked about it would answer
/// that nothing is running under it.
#[tokio::test]
async fn adopting_a_setup_that_is_still_running_is_refused_rather_than_captured_live() {
    let (ctx, vault, env) = machine(Lifecycle::Running, &["/srv/their-media"], "running");

    let refused = dispatch(
        Command::Migrate(MigrateAction::Act {
            mode: Mode::Adopt,
            confirmed: true,
        }),
        &ctx,
    )
    .await;
    assert!(refused.is_err(), "{refused:?}");
    assert!(vault.sources().is_empty(), "it captured a live database");
    assert!(!env.exists(), "a refusal wrote {}", env.display());
}

/// A setup that mounts nothing has nothing to capture, and is adopted without one.
///
/// The alternative would be filing an empty archive as a backup, which reads as
/// protection that was never there.
#[tokio::test]
async fn adopting_a_setup_that_mounts_nothing_takes_no_archive_and_still_goes_through() {
    let (ctx, vault, _) = machine(Lifecycle::Exited, &[], "unmounted");

    let found = adopting(&ctx, true).await;
    assert_eq!(
        found.as_ref().map(|read| read.stance),
        Some(Stance::Applied),
        "{found:?}"
    );
    assert_eq!(found.and_then(|read| read.backed_up), None);
    assert!(vault.sources().is_empty(), "it captured something");
}

/// A rehearsal captures nothing, the same as it writes nothing.
#[tokio::test]
async fn a_rehearsed_adoption_captures_nothing_and_leaves_the_setup_alone() {
    let (ctx, vault, env) = machine(Lifecycle::Exited, &["/srv/their-media"], "rehearsed");

    let found = adopting(&ctx, false).await;
    assert_eq!(
        found.as_ref().map(|read| read.stance),
        Some(Stance::Pending),
        "{found:?}"
    );
    assert!(vault.sources().is_empty(), "a rehearsal captured something");
    assert!(!env.exists(), "a rehearsal wrote {}", env.display());
}
