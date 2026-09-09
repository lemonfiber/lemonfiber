//! Taking over a setup that was already on this machine.
//!
//! The acting half of migration, and the only part of it that writes anything. What it
//! writes is one line of lemonfiber's own configuration — the Compose project it
//! manages. It starts nothing, stops nothing, moves nothing, and deletes nothing, so a
//! run that fails or is abandoned leaves the operator exactly the stack they had.
//!
//! Two things stop it, and they are the reason it exists as an act rather than a
//! setting. A database that has been through a **later** version than lemonfiber pins
//! cannot be opened by the older one, so adopting is refused outright — the attempt is
//! what damages it. A database an **earlier** version wrote will be upgraded on first
//! start, which nothing walks back, so adopting names the services it would happen to
//! and the paths their data sits in, and will not proceed until the operator confirms.

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
pub fn adopt(
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

    let path = path(ctx).ok_or_else(|| Box::new(store::Failure::Nowhere.problem()))?;
    store::set(&path, PROJECT_KEY, &project).map_err(|failure| Box::new(failure.problem()))?;

    Ok(AdoptReport {
        project: Some(project),
        stance: Stance::Applied,
        upgrades,
        back_up: mounts.to_vec(),
        ..AdoptReport::default()
    })
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
