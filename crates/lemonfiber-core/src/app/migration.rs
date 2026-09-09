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

use super::{Ctx, MigrateAction, Outcome};

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
        MigrateAction::Adopt { confirmed } => super::adopt::taking(ctx, confirmed)
            .await
            .map(Outcome::Adoption),
        MigrateAction::Import { confirmed } => {
            let found = looked(ctx).await;
            super::import::carry(ctx, &found.survey, &found.running, confirmed)
                .await
                .map(Outcome::Import)
        }
        MigrateAction::Replace { confirmed } => {
            let found = looked(ctx).await;
            super::replace::instead(ctx, &found.survey, &found.running, confirmed)
                .await
                .map(Outcome::Replacement)
        }
        MigrateAction::Beside { confirmed } => {
            let found = looked(ctx).await;
            super::beside::stand(ctx, &found.survey, confirmed)
                .await
                .map(Outcome::Beside)
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
pub(super) struct Looked {
    /// What the survey amounts to.
    pub survey: MigrationReport,
    /// Every host path the existing setup mounts.
    pub mounts: Vec<String>,
    /// Every container of every project that is not lemonfiber's.
    pub running: Vec<Container>,
}

/// What the engine and the manifest together say is here.
pub(super) async fn looked(ctx: &Ctx) -> Looked {
    let Ok(images) = ctx.images.images().await else {
        return Looked::nothing();
    };
    let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) else {
        return Looked::nothing();
    };

    let mut projects: BTreeSet<&str> = BTreeSet::new();
    for image in &images {
        projects.extend(
            image
                .projects
                .iter()
                .map(String::as_str)
                .filter(|project| !project.is_empty()),
        );
    }

    let mut seen: Vec<Container> = Vec::new();
    for project in projects {
        let Ok(containers) = ctx.engine.list(project).await else {
            return Looked::nothing();
        };
        seen.extend(containers);
    }

    let ours: Vec<Ours> = manifest
        .services
        .iter()
        .map(|service| Ours {
            service: service.id.clone(),
            image: service.image.clone(),
            tag: service.tag.clone(),
            port: service.port,
        })
        .collect();

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
