//! Taking over a setup that was already on this machine.
//!
//! The acting half of migration, and the only part of it that writes anything. What it
//! writes is one line of lemonfiber's own configuration — the Compose project it
//! manages — and one archive of the setup it is about to take over. It starts nothing,
//! stops nothing, moves nothing, and deletes nothing, so a run that fails or is
//! abandoned leaves the operator exactly the stack they had.
//!
//! The archive comes first, and is the reason this can refuse where it used to only
//! write. It covers the existing setup's own host paths, because lemonfiber's layout
//! holds nothing worth protecting until the takeover has happened — and it is taken
//! before the project key is recorded, so a capture that fails leaves nothing written
//! and nothing claimed.
//!
//! Two things stop it, and they are the reason it exists as an act rather than a
//! setting. A database that has been through a **later** version than lemonfiber pins
//! cannot be opened by the older one, so adopting is refused outright — the attempt is
//! what damages it. A database an **earlier** version wrote will be upgraded on first
//! start, which nothing walks back, so adopting names the services it would happen to
//! and the paths their data sits in, and will not proceed until the operator confirms.

use std::path::PathBuf;

use crate::config::{store, PROJECT_KEY};
use crate::error::{Diagnose, Problem};
use crate::model::{AdoptReport, CarryingReport, MigrationReport};
use crate::reconfigure::Stance;

use super::Ctx;

/// Take over the setup already here, or say why that cannot happen.
///
/// # Errors
///
/// Where there is nowhere configured to record the answer, or it cannot be written.
pub async fn adopt(
    ctx: &Ctx,
    survey: &MigrationReport,
    mounts: &[String],
    confirmed: bool,
) -> Result<AdoptReport, Box<Problem>> {
    let Some(project) = crate::migration::one_setup(survey) else {
        return Ok(nothing(survey));
    };

    let refused: Vec<&CarryingReport> = survey
        .carrying
        .iter()
        .filter(|service| service.refused)
        .collect();
    if let Some(first) = refused.first() {
        return Ok(AdoptReport {
            project: Some(project.clone()),
            stance: Stance::Blocked,
            refusal: Some(format!(
                "{} would have to open a database a later version wrote: {}",
                first.service, first.because
            )),
            ..AdoptReport::default()
        });
    }

    let upgrades: Vec<CarryingReport> = survey
        .carrying
        .iter()
        .filter(|service| service.backup_first)
        .cloned()
        .collect();

    if !confirmed {
        return Ok(AdoptReport {
            project: Some(project.clone()),
            upgrades,
            back_up: mounts.to_vec(),
            stance: Stance::Pending,
            ..AdoptReport::default()
        });
    }

    // Resolved before the capture rather than after it, so a run with nowhere to
    // record its answer refuses without first spending the time and the disk on an
    // archive for a takeover that was never going to happen.
    let path = path(ctx).ok_or_else(|| Box::new(store::Failure::Nowhere.problem()))?;

    let backed_up = kept(ctx, &project, mounts).await?;

    store::set(&path, PROJECT_KEY, &project).map_err(|failure| Box::new(failure.problem()))?;

    Ok(AdoptReport {
        project: Some(project),
        stance: Stance::Applied,
        upgrades,
        back_up: mounts.to_vec(),
        backed_up,
        ..AdoptReport::default()
    })
}

/// Capture the setup about to be taken over, and say where it was written.
///
/// Nothing is captured where the setup mounts nothing: there is no tree to copy, and
/// an empty archive filed as a backup would read as protection that was never there.
///
/// Where there is something, a failure to capture it fails the adoption. That is the
/// whole point of taking it — an operator who is told their configuration was
/// protected, and finds later that it was not, is worse off than one who was refused.
async fn kept(
    ctx: &Ctx,
    project: &str,
    mounts: &[String],
) -> Result<Option<PathBuf>, Box<Problem>> {
    if mounts.is_empty() {
        return Ok(None);
    }
    let report = super::backup::existing(ctx, project, mounts).await?;
    Ok(Some(report.path))
}

/// What is answered where there is no one setup to take over.
fn nothing(survey: &MigrationReport) -> AdoptReport {
    let refused = if survey.read {
        "there is no single setup here holding services lemonfiber runs, so there is \
         nothing to take over — name what you meant, or stand a stack up instead"
    } else {
        "what is on this machine could not be read, and adopting a setup that could not \
         be looked at would be standing over whatever is actually there"
    };
    AdoptReport {
        stance: Stance::Blocked,
        refusal: Some(refused.to_owned()),
        ..AdoptReport::default()
    }
}

/// Where the answer is recorded.
fn path(ctx: &Ctx) -> Option<std::path::PathBuf> {
    ctx.settings.env_file.clone()
}

#[cfg(test)]
mod tests {
    use super::nothing;
    use crate::model::MigrationReport;

    /// A survey that looked and found nothing it could act on.
    fn found() -> MigrationReport {
        MigrationReport {
            read: true,
            ..MigrationReport::default()
        }
    }

    /// Both refusals, in one place. A survey that looked and found no single setup and
    /// one that could not look are different facts, and only one of them says anything
    /// about what is on the machine.
    #[test]
    fn what_cannot_be_adopted_says_which_of_the_two_it_is() {
        let looked = nothing(&found());
        let said = looked.refusal.unwrap_or_default();
        assert!(said.contains("no single setup here"), "{said}");

        let blind = nothing(&MigrationReport::default());
        let said = blind.refusal.unwrap_or_default();
        assert!(said.contains("could not be read"), "{said}");
    }
}
