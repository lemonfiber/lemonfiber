//! Surveying what is already on this machine, before anything is proposed.
//!
//! The gathering half: this reaches the engine and the manifest, and hands what it read
//! to [`crate::migration`], which decides what it amounts to. Nothing here writes.
//!
//! Every way of failing to look lands on the same answer — a report that says it did not
//! read. That the survey found nothing and that the survey could not run are different
//! facts, and only one of them makes it safe to stand a stack up here.

use std::collections::BTreeSet;

use crate::error::Problem;
use crate::migration::{carrying, outside_compose, surveyed, unread, Pinned, Wanted};
use crate::model::MigrationReport;
use crate::ports::docker::Container;

use super::{Ctx, MigrateAction};

/// Answer what is already here.
///
/// # Errors
///
/// Never in practice: a survey that cannot look reports that it could not, rather than
/// refusing. The result carries the failure so the caller stays uniform with the other
/// reads.
pub async fn survey(ctx: &Ctx, action: MigrateAction) -> Result<MigrationReport, Box<Problem>> {
    match action {
        MigrateAction::Survey => Ok(looked(ctx).await),
    }
}

/// What the engine and the manifest together say is here.
async fn looked(ctx: &Ctx) -> MigrationReport {
    let Ok(images) = ctx.images.images().await else {
        return unread();
    };
    let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) else {
        return unread();
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
            return unread();
        };
        seen.extend(containers);
    }

    let known: Vec<String> = manifest
        .services
        .iter()
        .map(|service| service.id.clone())
        .collect();
    let wanted: Vec<Wanted> = manifest
        .services
        .iter()
        .filter_map(|service| {
            service.port.map(|port| Wanted {
                service: service.id.clone(),
                port,
            })
        })
        .collect();

    let pulled: Vec<String> = manifest
        .services
        .iter()
        .map(|service| service.image.clone())
        .collect();

    let pinned: Vec<Pinned> = manifest
        .services
        .iter()
        .map(|service| Pinned {
            service: service.id.clone(),
            image: service.image.clone(),
            tag: service.tag.clone(),
        })
        .collect();

    let mut report = surveyed(&ctx.settings.project, &seen, &wanted, &known);
    report.unsupported.extend(outside_compose(&images, &pulled));
    for standing in &report.standing {
        report
            .carrying
            .extend(carrying(&images, &standing.project, &pinned));
    }
    report
}
