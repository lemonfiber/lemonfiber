//! Standing lemonfiber up in place of a setup that is already running.
//!
//! The most destructive of the four modes, and the only one that touches what is
//! running. It stops the containers of the existing project and **deletes nothing** —
//! not an image, not a volume, not a file — so the operator can start their old stack
//! again if they change their mind. That is the whole of what makes this reversible,
//! and a guard holds this file to it.
//!
//! Stopped one container at a time by the ids the survey read from the engine, rather
//! than through their Compose file: lemonfiber has never seen that file, and a stop
//! aimed at a project it cannot read would be a stop aimed at a guess.
//!
//! **The yes is the offer, and nothing else is.** Asked without one it names what it
//! would stop, names that offer, and stops nothing. Answered with the name, it builds
//! the offer again from what is running now and stops only where the two are the same
//! — so a container started, or a project changed, between reading and agreeing is
//! refused by name rather than stopped unseen. A bare confirmation would be agreement
//! from somebody who had not read what would stop, which is the one thing this mode
//! cannot afford.

use crate::error::codes::migrate::OFFER_MOVED;
use crate::error::{Problem, Remedy, Severity, State};
use crate::model::{MigrationReport, ReplaceReport};
use crate::ports::docker::Container;
use crate::reconfigure::Stance;

use super::Ctx;

/// Stand in place of what is already here, or say what that would stop.
///
/// # Errors
///
/// Returns a [`Problem`] where the offer answered is not the one standing now. What
/// could not be stopped is reported rather than refused, so the operator can see which
/// half of their stack is still up.
pub async fn instead(
    ctx: &Ctx,
    survey: &MigrationReport,
    running: &[Container],
    offer: Option<&str>,
) -> Result<ReplaceReport, Box<Problem>> {
    let Some(project) = crate::migration::one_setup(survey) else {
        return Ok(refused(survey));
    };

    // Every container of that project, which is never empty: the project is standing
    // here precisely because these containers were read from the engine.
    let theirs: Vec<&Container> = running
        .iter()
        .filter(|container| container.project == project)
        .collect();

    let mut names: Vec<String> = theirs
        .iter()
        .map(|container| container.service.clone())
        .collect();
    names.sort();
    let agreement = agreement(&project, &names);

    let Some(given) = offer else {
        return Ok(ReplaceReport {
            project: Some(project),
            would_stop: names,
            agreement,
            stance: Stance::Pending,
            ..ReplaceReport::default()
        });
    };
    if given != agreement {
        return Err(Box::new(another_offer(given, &agreement, &project, &names)));
    }

    let mut stopped = Vec::new();
    let mut left = Vec::new();
    for container in theirs {
        let asked = vec!["docker".to_owned(), "stop".to_owned(), container.id.clone()];
        match ctx.seams.runner.run(&asked).await {
            Ok(output) if output.status == Some(0) => stopped.push(container.service.clone()),
            _ => left.push(container.service.clone()),
        }
    }

    Ok(ReplaceReport {
        project: Some(project),
        would_stop: names,
        agreement,
        stopped,
        still_running: left,
        stance: Stance::Applied,
        ..ReplaceReport::default()
    })
}

/// The name an offer to stand in place of `project` goes by.
///
/// Over the project and every service it would stop, in the order they are read, so
/// a service started or gone, or a different project standing here, is a different
/// offer.
#[must_use]
pub fn agreement(project: &str, would_stop: &[String]) -> String {
    let mut words = vec![project];
    words.extend(would_stop.iter().map(String::as_str));
    crate::agreement::over(&words)
}

/// What is answered where there is no one setup to stand in place of.
fn refused(survey: &MigrationReport) -> ReplaceReport {
    let why = if survey.read {
        "there is no single setup here holding services lemonfiber runs, so there is \
         nothing to stand in place of — name what you meant, or stand a stack up instead"
    } else {
        "what is on this machine could not be read, and stopping a setup that could not \
         be looked at would be stopping something nobody has seen"
    };
    ReplaceReport {
        stance: Stance::Blocked,
        refusal: Some(why.to_owned()),
        ..ReplaceReport::default()
    }
}

/// The refusal for an answer to an offer that is not the one standing now.
///
/// Names what replacing would stop now, because the answer carries only the name of
/// what was read and an operator told only that something moved cannot tell a service
/// started since from a different project standing here. Both names are said, for
/// the reason a stale repair offer says both.
fn another_offer(agreed: &str, stands: &str, project: &str, would_stop: &[String]) -> Problem {
    crate::agreement::moved(
        Problem::new(
            OFFER_MOVED,
            Severity::Warning,
            "What you agreed to is not what standing in place of this setup would stop now",
            format!(
                "The offer you answered was {agreed}, and a fresh look offers {stands}. \
                 Something has started, stopped or changed since you read it, so nothing \
                 was stopped."
            ),
            Remedy::new("Ask what replacing would stop again, and read what it says now"),
        )
        .in_state(State::Guided)
        .with_detail(format!(
            "standing in place of {project} would now stop {}",
            would_stop.join(", ")
        )),
    )
}

#[cfg(test)]
mod tests;
