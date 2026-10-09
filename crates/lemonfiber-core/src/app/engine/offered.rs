//! The offer a restart answers, and the refusal of one that has moved.
//!
//! A restart is called by a program on a schedule as well as after asking somebody, so
//! a restart carrying no offer acts as it always has. One carrying the offer its
//! rehearsal made is carried out against the services that offer named, or refused:
//! a restart rehearsed against one set of services is never carried out against
//! another.

use super::{addressed, lock, worked};
use crate::agreement::over;
use crate::app::{Command, Ctx, Outcome};
use crate::error::codes::life::RESTART_MOVED;
use crate::error::{Problem, Remedy, State};
use crate::model::LifecycleReport;
use crate::stack::compose::Action;

/// Restart what the forms hold, or the services named within them, answering the offer
/// its rehearsal made where the restart carries one back.
///
/// Handed the command whole so the dispatch table keeps one line for it; anything but a
/// restart is answered as one that names nothing to restart.
///
/// # Errors
///
/// What [`answering`] returns.
pub(crate) async fn restarted(ctx: &Ctx, restart: Command) -> Result<Outcome, Box<Problem>> {
    let (forms, services, offer) = match restart {
        Command::Restart {
            forms,
            services,
            offer,
        } => (forms, services, offer),
        _ => (Vec::new(), Vec::new(), None),
    };
    answering(ctx, &forms, &Action::Restart(services), offer.as_deref())
        .await
        .map(Outcome::Lifecycle)
}

/// A lifecycle action, answering the offer a rehearsal of it made where one is carried
/// back: refused, before anything runs, where the offer built now is another.
///
/// Only a restart answers one. Every other action is carried out as without one.
///
/// # Errors
///
/// What [`super::lifecycle`] returns, and the moved offer's refusal.
pub(crate) async fn answering(
    ctx: &Ctx,
    forms: &[String],
    action: &Action,
    offer: Option<&str>,
) -> Result<LifecycleReport, Box<Problem>> {
    // Claimed around the whole operation, and given back whether it worked or not —
    // an early return between the two would leave the stack claimed by a run that has
    // already finished, which is the one way this can be worse than no lock at all.
    //
    // The claim is recorded under the Compose verb rather than under the command the
    // surface was given, because that is the word the next run to ask is shown and it
    // has to mean something to somebody who did not type it.
    let claim = lock::claimed(ctx, action.name()).await?;
    let outcome = worked(ctx, forms, action, offer).await;
    lock::released(claim).await;
    outcome
}

/// Put the offer a restart answers on its report, refusing an answered one that is not
/// it. Nothing for any other action.
pub(super) fn answered(
    action: &Action,
    report: &mut LifecycleReport,
    offer: Option<&str>,
) -> Result<(), Box<Problem>> {
    if !matches!(action, Action::Restart(_)) {
        return Ok(());
    }
    let services = addressed(action, &report.plan);
    let standing = restarting(&services);
    if let Some(answered) = offer.filter(|answered| *answered != standing) {
        return Err(Box::new(moved(answered, &standing, &services)));
    }
    report.offer = Some(standing);
    Ok(())
}

/// The offer a restart of `services` answers: their names, in the order given.
fn restarting(services: &[String]) -> String {
    let named: Vec<&str> = services.iter().map(String::as_str).collect();
    over(&named)
}

/// A restart answering an offer that is not the one standing now.
fn moved(answered: &str, standing: &str, services: &[String]) -> Problem {
    Problem::new(
        RESTART_MOVED,
        "That agreement was given for a different restart",
        format!(
            "It answered {answered}, and a restart now would restart {}, which names {standing}. \
             Nothing was restarted.",
            services.join(", ")
        ),
        Remedy::new("Rehearse the restart again, and answer the offer it gives now"),
    )
    .in_state(State::Guided)
}

#[cfg(test)]
mod tests;
