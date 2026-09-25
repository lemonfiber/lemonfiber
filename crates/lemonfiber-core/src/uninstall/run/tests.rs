//! Driving a removal against a machine a test wrote down.
//!
//! Every case here reaches the real survey and the real removal. The engine, the
//! image listing, the walk and the eraser are the seams; nothing else is stood in
//! for, so what these assert is what an operator would get.
//!
//! A run that refused answers with nothing rather than with an empty manifest, and
//! every assertion is made on that `Option` — so a case whose run refused fails on
//! the assertion it meant to make instead of on a shape it had to unwrap.

use std::path::PathBuf;
use std::sync::Arc;

use lemonfiber_fixtures::erasing::Erasing;
use lemonfiber_fixtures::pulled::Pulled;
use lemonfiber_fixtures::support::{spoke, Recording, Reporting, SeedFs};
use lemonfiber_fixtures::walking::Walking;
use lemonfiber_ports::docker::{Health, Lifecycle};
use lemonfiber_ports::occupancy::Occupant;

use super::uninstall;
use crate::app::{Ctx, Removing};
use crate::config::Settings;
use crate::ports::filesystem::{Eraser, FsKind, StorageFacts};
use crate::ports::Runner;
use crate::test_support::{a_context, nowhere};
use crate::uninstall::{
    Item, Left, Manifest, Removal, Sort, Tier, ANOTHER_READING, NEEDS_AGREEING,
};

/// The data location every case here is about.
const ROOT: &str = "/srv/media";

/// One image this stack declares, named once so a case does not spell the tag twice.
const SONARR: &str = "lscr.io/linuxserver/sonarr:4.0.15";

/// Settings naming a project, a layout and a data location — the three a survey
/// reads before it asks anything.
fn settings() -> Settings {
    Settings {
        project: "lemonfiber".to_owned(),
        env_file: Some(PathBuf::from("/cfg/lemonfiber/.env")),
        stack_dir: Some(PathBuf::from("/data/lemonfiber/stack")),
        data_root: Some(PathBuf::from(ROOT)),
        ..Settings::default()
    }
}

/// A file the walk reports at a path beneath the data location.
fn file(path: &str, bytes: u64) -> Occupant {
    Occupant {
        path: PathBuf::from(path),
        bytes,
        identity: None,
    }
}

/// A tree holding only what the stack itself writes.
fn only_ours() -> Vec<Occupant> {
    vec![
        file("/srv/media/downloads/A.Show/a.mkv", 1_000),
        file("/srv/media/media/tv/A Show/S01E01.mkv", 2_000),
    ]
}

/// A filesystem answering every location, over an ordinary local disk.
fn a_filesystem() -> Arc<SeedFs> {
    over(FsKind::Linking("apfs".to_owned()), false)
}

/// A filesystem reporting the data location on a given sort of volume.
fn over(kind: FsKind, removable: bool) -> Arc<SeedFs> {
    Arc::new(SeedFs::keyed(None, None).with_facts(StorageFacts {
        point: PathBuf::from(ROOT),
        kind,
        removable,
        available: 100,
        total: 1_000,
    }))
}

/// A machine with a stack the engine is holding, one image pulled for it, and
/// nothing but the stack's own files on the disk.
fn a_machine() -> Ctx {
    kept(
        running(Lifecycle::Running, Health::Healthy)
            .with_images(Pulled::holding(vec![Pulled::image(
                SONARR,
                400,
                &["lemonfiber"],
            )]))
            .with_eraser(Erasing::willing()),
    )
}

/// The same machine, with somewhere to put the backup a destructive tier takes first.
///
/// Every reading here is of a machine that has one: a removal that destroys
/// configuration captures it before anything goes, so a fixture with nowhere to keep an
/// archive would be testing the refusal rather than the removal. The refusal has a test
/// of its own, which is the machine without this.
fn kept(ctx: Ctx) -> Ctx {
    crate::app::fixtures::keeping(ctx, &Arc::new(crate::app::fixtures::FakeArchive::roomy()))
}

/// The same machine, with its services in a given state.
fn running(lifecycle: Lifecycle, health: Health) -> Ctx {
    a_context()
        .settings(settings())
        .engine(Arc::new(Reporting::holding(&["sonarr"], lifecycle, health)))
        .build()
        .with_filesystem(a_filesystem())
        .with_images(Pulled::holding(Vec::new()))
        .with_occupancy(Walking::holding(only_ours()))
        .with_eraser(Erasing::willing())
}

/// What a run came to, or nothing where it refused.
async fn answer(ctx: &Ctx, asked: Removing) -> Option<crate::uninstall::Uninstall> {
    uninstall(ctx, asked).await.ok()
}

/// What a reading of one tier found, or nothing where it refused.
async fn read(ctx: &Ctx, tier: Tier) -> Option<Manifest> {
    answer(ctx, Removing::surveying(tier))
        .await
        .map(|answered| answered.manifest)
}

/// What a confirmed run came to, answering the reading it printed where the tier
/// takes one.
async fn confirmed(ctx: &Ctx, tier: Tier) -> Option<Removal> {
    let agreement = read(ctx, tier).await.map(|manifest| manifest.agreement);
    let asked = Removing::surveying(tier)
        .confirmed(true)
        .agreeing(agreement.filter(|_| tier.needs_its_own_agreement()));
    answer(ctx, asked).await.map(|answered| answered.removal)
}

/// The lines of a manifest whose name holds this text.
fn named(manifest: &Manifest, contains: &str) -> Vec<Item> {
    manifest
        .items
        .iter()
        .filter(|item| item.name.contains(contains))
        .cloned()
        .collect()
}

/// The names of the lines a manifest says are going.
fn going(manifest: &Manifest) -> Vec<String> {
    manifest
        .items
        .iter()
        .filter(|item| item.goes())
        .map(|item| item.name.clone())
        .collect()
}

/// What a removal could not take, or an empty list where it took everything.
fn left(removal: &Removal) -> Vec<Left> {
    match removal {
        Removal::Partial { left, .. } => left.clone(),
        Removal::Surveyed | Removal::Confirmed | Removal::Complete { .. } => Vec::new(),
    }
}

// --- The manifest ------------------------------------------------------------

// --- The library -------------------------------------------------------------

// --- Files the stack did not write -------------------------------------------

// --- Configuration and credentials -------------------------------------------

// --- What lemonfiber cannot remove -------------------------------------------

// --- A machine that is broken ------------------------------------------------

// --- Images shared with other projects ---------------------------------------

// --- No privilege escalation --------------------------------------------------

// --- Downloads still coming down ----------------------------------------------

// --- Rehearsal and reading ----------------------------------------------------

mod agreeing;
mod reading;
