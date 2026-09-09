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
    let Some(project) = adoptable(survey) else {
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
            refused: Some(format!(
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
            rehearsed: true,
            ..AdoptReport::default()
        });
    }

    let path = path(ctx).ok_or_else(|| Box::new(store::Failure::Nowhere.problem()))?;
    store::set(&path, PROJECT_KEY, &project).map_err(|failure| Box::new(failure.problem()))?;

    Ok(AdoptReport {
        project: Some(project),
        adopted: true,
        upgrades,
        back_up: mounts.to_vec(),
        ..AdoptReport::default()
    })
}

/// The one project that could be taken over, where there is exactly one.
///
/// Exactly one, because adopting is a choice about which stack lemonfiber becomes the
/// surface for, and a machine holding two of them is one where nobody but the operator
/// can say which they meant. A project holding nothing lemonfiber runs is not a
/// candidate at all.
fn adoptable(survey: &MigrationReport) -> Option<String> {
    let mut candidates = survey
        .standing
        .iter()
        .filter(|project| project.services.iter().any(|service| service.adoptable));
    let first = candidates.next()?;
    if candidates.next().is_some() {
        return None;
    }
    Some(first.project.clone())
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
        refused: Some(refused.to_owned()),
        ..AdoptReport::default()
    }
}

/// Where the answer is recorded.
fn path(ctx: &Ctx) -> Option<std::path::PathBuf> {
    ctx.settings.env_file.clone()
}

/// Everything adopting needs from the machine, gathered once.
pub async fn taking(ctx: &Ctx, confirmed: bool) -> Result<AdoptReport, Box<Problem>> {
    let found = super::migration::looked(ctx).await;
    adopt(ctx, &found.survey, &found.mounts, confirmed)
}

#[cfg(test)]
mod tests {
    use super::{adoptable, nothing};
    use crate::model::{MigrationReport, OccupantReport, StandingReport};

    /// One project standing here, holding the named services.
    fn standing(project: &str, services: &[(&str, bool)]) -> StandingReport {
        StandingReport {
            project: project.to_owned(),
            services: services
                .iter()
                .map(|(service, adoptable)| OccupantReport {
                    service: (*service).to_owned(),
                    running: true,
                    ports: Vec::new(),
                    adoptable: *adoptable,
                })
                .collect(),
        }
    }

    /// A survey that looked and found these.
    fn found(standing: Vec<StandingReport>) -> MigrationReport {
        MigrationReport {
            read: true,
            standing,
            ..MigrationReport::default()
        }
    }

    #[test]
    fn the_one_setup_holding_our_services_is_the_one_taken_over() {
        let survey = found(vec![standing("media", &[("sonarr", true)])]);
        assert_eq!(adoptable(&survey), Some("media".to_owned()));
    }

    /// Somebody's database and cache are not a stack lemonfiber was asked to become
    /// the surface for.
    #[test]
    fn a_project_holding_nothing_of_ours_is_not_a_candidate() {
        let survey = found(vec![standing("shop", &[("postgres", false)])]);
        assert_eq!(adoptable(&survey), None);
    }

    /// Two of them is a question only the operator can answer, and guessing would make
    /// lemonfiber the surface for a stack they did not mean.
    #[test]
    fn two_setups_holding_our_services_is_not_a_choice_lemonfiber_makes() {
        let survey = found(vec![
            standing("media", &[("sonarr", true)]),
            standing("archive", &[("radarr", true)]),
        ]);
        assert_eq!(adoptable(&survey), None);
    }

    /// A project of ours beside one of theirs is still exactly one candidate.
    #[test]
    fn an_unrelated_project_beside_ours_does_not_make_it_ambiguous() {
        let survey = found(vec![
            standing("media", &[("sonarr", true)]),
            standing("shop", &[("redis", false)]),
        ]);
        assert_eq!(adoptable(&survey), Some("media".to_owned()));
    }

    /// Both refusals, in one place. A survey that looked and found no single setup and
    /// one that could not look are different facts, and only one of them says anything
    /// about what is on the machine.
    #[test]
    fn what_cannot_be_adopted_says_which_of_the_two_it_is() {
        let looked = nothing(&found(vec![]));
        let said = looked.refused.unwrap_or_default();
        assert!(said.contains("no single setup here"), "{said}");

        let blind = nothing(&MigrationReport::default());
        let said = blind.refused.unwrap_or_default();
        assert!(said.contains("could not be read"), "{said}");
    }
}
