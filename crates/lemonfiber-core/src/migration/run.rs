//! Surveying what is already on this machine, before anything is proposed.
//!
//! The gathering half: this reaches the engine and the manifest, and hands what it read
//! to [`crate::migration`], which decides what it amounts to. Nothing here writes.
//!
//! Every way of failing to look lands on the same answer — a report that says it did not
//! read. That the survey found nothing and that the survey could not run are different
//! facts, and only one of them makes it safe to stand a stack up here.

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::error::Problem;
use crate::migration::{surveyed, unread, Ours};
use crate::model::MigrationReport;
use crate::ports::docker::Container;
use crate::ports::filesystem::StorageFacts;

use crate::migration::mode::Mode;

use crate::app::{Ctx, MigrateAction, Outcome};

/// Answer what is already here.
///
/// # Errors
///
/// Never in practice: a survey that cannot look reports that it could not, rather than
/// refusing. The result carries the failure so the caller stays uniform with the other
/// reads.
pub async fn migrating(ctx: &Ctx, action: MigrateAction) -> Result<Outcome, Box<Problem>> {
    match action {
        MigrateAction::Survey => Ok(Outcome::Migration(looked(ctx).await.survey)),
        MigrateAction::Act { mode, confirmed } => acting(ctx, mode, confirmed).await,
    }
}

/// Carry out one of the four things that may be done about what was found.
///
/// Each looks first. What they do about what they found is where they differ, and the
/// looking is one answer rather than four so that two of them cannot disagree about
/// what is on the machine.
async fn acting(ctx: &Ctx, mode: Mode, confirmed: bool) -> Result<Outcome, Box<Problem>> {
    let found = looked(ctx).await;
    match mode {
        Mode::Adopt => crate::app::adopt::adopt(ctx, &found.survey, &found.mounts, confirmed)
            .await
            .map(Outcome::Adoption),
        Mode::Import => crate::app::import::carry(ctx, &found.survey, &found.running, confirmed)
            .await
            .map(Outcome::Import),
        Mode::Beside => crate::app::beside::stand(ctx, &found.survey, confirmed)
            .await
            .map(Outcome::Beside),
        Mode::Replace => {
            crate::app::replace::instead(ctx, &found.survey, &found.running, confirmed)
                .await
                .map(Outcome::Replacement)
        }
    }
}

/// What filesystem each path the existing setup mounts actually sits on.
///
/// Asked of every distinct host path once. `describe` is a read — it says what a
/// filesystem is, not what is in it — and it is the only thing a survey asks of that
/// seam, which the architecture guard holds it to.
async fn mounted(ctx: &Ctx, seen: &[Container]) -> Vec<(PathBuf, StorageFacts)> {
    let paths: BTreeSet<&PathBuf> = seen
        .iter()
        .filter(|container| container.project != ctx.settings.project)
        .flat_map(|container| container.mounts.iter())
        .collect();

    let mut found = Vec::new();
    for path in paths {
        found.push((path.clone(), ctx.storage().describe(path).await));
    }
    found
}

/// Everything one look at this machine found.
///
/// One struct rather than a tuple that grew: the mounts are what adopting names to back
/// up, the containers are what standing in place of it stops, and reading the engine
/// once is what keeps them the same answer.
pub(crate) struct Looked {
    /// What the survey amounts to.
    pub survey: MigrationReport,
    /// Every host path the existing setup mounts.
    pub mounts: Vec<String>,
    /// Every container of every project that is not lemonfiber's.
    pub running: Vec<Container>,
}

/// What the engine and the manifest together say is here.
pub(crate) async fn looked(ctx: &Ctx) -> Looked {
    let Ok(images) = ctx.images.images().await else {
        return Looked::nothing();
    };
    let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) else {
        return Looked::nothing();
    };

    // The same walk a start's port pre-flight makes, shared rather than written
    // twice: two enumerations of the projects on one machine is two ways for the
    // survey and the pre-flight to disagree about what is standing here.
    let Some(seen) = crate::app::preflight::every_container(ctx, &images).await else {
        return Looked::nothing();
    };

    let ours: Vec<Ours> = crate::migration::pins(&manifest);

    let mounted = mounted(ctx, &seen).await;
    let paths = mounted
        .iter()
        .map(|(path, _)| path.display().to_string())
        .collect();

    Looked {
        survey: surveyed(&ctx.settings.project, &seen, &images, &ours, &mounted),
        mounts: paths,
        running: seen,
    }
}

impl Looked {
    /// What a look that could not look answers with.
    fn nothing() -> Self {
        Self {
            survey: unread(),
            mounts: Vec::new(),
            running: Vec::new(),
        }
    }
}
