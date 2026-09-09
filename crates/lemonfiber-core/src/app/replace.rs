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
//! Unconfirmed it names what it would stop and stops nothing.

use crate::error::Problem;
use crate::model::{MigrationReport, ReplaceReport};
use crate::ports::docker::Container;

use super::Ctx;

/// Stand in place of what is already here, or say why that cannot happen.
///
/// # Errors
///
/// Never in practice: what could not be stopped is reported rather than refused, so the
/// operator can see which half of their stack is still up.
pub async fn instead(
    ctx: &Ctx,
    survey: &MigrationReport,
    running: &[Container],
    confirmed: bool,
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

    if !confirmed {
        return Ok(ReplaceReport {
            project: Some(project),
            would_stop: names,
            rehearsed: true,
            ..ReplaceReport::default()
        });
    }

    let mut stopped = Vec::new();
    let mut left = Vec::new();
    for container in theirs {
        let asked = vec!["docker".to_owned(), "stop".to_owned(), container.id.clone()];
        match ctx.runner.run(&asked).await {
            Ok(output) if output.status == Some(0) => stopped.push(container.service.clone()),
            _ => left.push(container.service.clone()),
        }
    }

    Ok(ReplaceReport {
        project: Some(project),
        would_stop: names,
        stopped,
        still_running: left,
        applied: true,
        ..ReplaceReport::default()
    })
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
        refused: Some(why.to_owned()),
        ..ReplaceReport::default()
    }
}

#[cfg(test)]
mod tests {
    use super::refused;
    use crate::model::MigrationReport;

    /// Having looked and found nothing, and not having looked, are different facts.
    #[test]
    fn what_cannot_be_replaced_says_which_of_the_two_it_is() {
        let looked = refused(&MigrationReport {
            read: true,
            ..MigrationReport::default()
        })
        .refused
        .unwrap_or_default();
        assert!(looked.contains("no single setup"), "{looked}");
        let blind = refused(&MigrationReport::default())
            .refused
            .unwrap_or_default();
        assert!(blind.contains("could not be read"), "{blind}");
    }
}
