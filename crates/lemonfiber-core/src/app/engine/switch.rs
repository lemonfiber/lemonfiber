//! Making named forms the active set, moving as little as possible.
//!
//! Narrowing is not stopping and starting again. A service the old shape of the
//! stack and the new one both hold keeps running, because restarting it would
//! interrupt work the operator did not ask to interrupt — a download in flight, a
//! library scan half done. So the new closure is resolved, what is up is surveyed,
//! and only the difference in each direction is acted on.
//!
//! Deciding *what* moves is pure and lives here as [`moved`]; running the two
//! Compose invocations that carry it out is the rest.

use lemonfiber_manifest::Manifest;

use super::{compose, lock, settled_into, Composed};
use crate::app::Ctx;
use crate::docker::{stopping_order, survey, Service, State};
use crate::error::{Diagnose, Problem};
use crate::model::{LifecycleReport, Switched};
use crate::stack::closure::Plan;
use crate::stack::compose::{build, Action};

/// What this reports itself as having done.
///
/// Not a Compose subcommand, unlike every other lifecycle action: a switch is up to
/// two of them, and naming it after either would describe half of what happened.
const SWITCH: &str = "switch";

/// Make the named forms the active set, stopping only what falls outside them.
///
/// Stopping happens first. What is stopped is by definition outside the new
/// closure, and freeing its ports and its network namespace before the new set
/// starts is what keeps a switch from failing on an address the old shape still
/// held. A stop that fails stops the switch: starting the new set over one that
/// would not go down is how two shapes of the stack come to be running at once.
///
/// A rehearsal still surveys — reading what is running is not acting on it — and
/// then reports both invocations without running either, because what an operator
/// wants from a rehearsed switch is precisely the list of what it would stop. That
/// makes this the one lifecycle command whose rehearsal needs a reachable engine,
/// and it is the right trade: what a switch would move is a statement about what is
/// running, and an engine that cannot be asked leaves it nothing to say. Refusing
/// beats reporting that nothing would stop, which would be a claim about a stack it
/// never managed to look at.
///
/// # Errors
///
/// Returns the [`Problem`] a surface should render when the stack cannot be read,
/// the forms cannot be resolved, the engine cannot be reached, or the services that
/// were started never became usable.
pub(crate) async fn switch(ctx: &Ctx, forms: &[String]) -> Result<LifecycleReport, Box<Problem>> {
    // Claimed for the same reason `lifecycle` claims: a switch stops services and
    // starts others, and two of them against one stack interleave a teardown with a
    // start. Given back whether it worked or not — an early return between the two
    // would leave the stack claimed by a run that has already finished.
    //
    // Recorded under its own name rather than under either of the two Compose
    // invocations it runs, because a run waiting behind it is shown that word and
    // "down" would be half of what is in the way.
    let claim = lock::claimed(ctx, SWITCH).await?;
    let outcome = moving(ctx, forms).await;
    lock::released(ctx, claim).await;
    outcome
}

/// The switch itself, with the stack already claimed for it.
async fn moving(ctx: &Ctx, forms: &[String]) -> Result<LifecycleReport, Box<Problem>> {
    // A switch is two lifecycle commands in a coat, so it owes the same pre-flight
    // they do. It does not go through the one they share, which is exactly how a
    // guard comes to hold everywhere but the path nobody remembered.
    super::remote::verified(ctx).await?;

    let Composed {
        manifest,
        plan,
        command,
        stack,
        stack_edits,
    } = compose(ctx, forms, &Action::Up)?;

    // Surveyed across every profile the stack declares rather than across the new
    // closure. The services a switch has to stop are the ones the new closure does
    // not hold, so asking only about the new closure would never see them.
    let declared: Vec<String> = manifest
        .profiles
        .iter()
        .map(|profile| profile.id.clone())
        .collect();
    let containers = ctx
        .seams
        .engine
        .list(&ctx.settings.project)
        .await
        .map_err(|err| Box::new(err.problem()))?;
    let running = survey(&manifest, &declared, &containers, ctx.settings.protocols);

    let mut switched = moved(&running, &plan.services);
    if !switched.stopped.is_empty() {
        switched.stop_command = Some(build(
            &leaving(&manifest, &switched.stopped),
            &ctx.settings,
            &stack,
            &Action::Stop(switched.stopped.clone()),
            ctx.environment,
        ));
    }
    let stopping = switched.stop_command.clone();

    // Only what is about to start. A service the switch keeps running already holds
    // its port and cannot clash with itself, and one it is stopping is giving a port
    // up rather than asking for it.
    let port_conflicts =
        crate::app::preflight::conflicting_ports(ctx, &manifest, &switched.started).await;

    let mut report = LifecycleReport {
        action: SWITCH.to_owned(),
        plan,
        command: command.clone(),
        rehearsed: ctx.dry_run,
        status: None,
        services: Vec::new(),
        condition: None,
        stack_edits,
        port_conflicts,
        forwarding: None,
        switched: Some(switched),
        held: None,
    };

    if ctx.dry_run {
        return Ok(report);
    }

    if let Some(stopping) = stopping {
        let output = ctx
            .seams
            .runner
            .run(&stopping)
            .await
            .map_err(|err| Box::new(err.problem()))?;
        report.status = output.status;
        if !output.succeeded() {
            return Ok(report);
        }
    }

    let started = ctx
        .seams
        .runner
        .run(&command)
        .await
        .map_err(|err| Box::new(err.problem()))?;
    report.status = started.status;
    if !started.succeeded() {
        return Ok(report);
    }

    // Waited for last rather than inside a branch, because what a switch started is
    // the same thing bringing a form up starts, and it is owed the same wait.
    settled_into(ctx, &manifest, &mut report).await?;
    Ok(report)
}

/// What a switch moves, given what is running and what the new closure holds.
///
/// Pure, so the decision that matters can be read and tested without a daemon:
/// narrowing comes down entirely to which of these three lists a service lands in.
fn moved(running: &[Service], holds: &[String]) -> Switched {
    let up: Vec<&str> = running
        .iter()
        .filter(|service| service.state.stoppable())
        .map(|service| service.id.as_str())
        .collect();

    // Both of these are read out of the new closure rather than out of the survey,
    // so they arrive in the order the stack declares its services — the same order
    // the plan beside them in the report is in. They are exact complements: a
    // service the new shape holds is either already up or about to be started, and
    // one the operating system owns is neither.
    let kept: Vec<String> = holds
        .iter()
        .filter(|id| up.contains(&id.as_str()))
        .cloned()
        .collect();
    let started: Vec<String> = holds
        .iter()
        .filter(|id| !up.contains(&id.as_str()))
        .filter(|id| !host_managed(running, id.as_str()))
        .cloned()
        .collect();

    // Ordered so that whatever depends on a service goes down before it does — the
    // torrent client before the tunnel whose network it is using. The survey's own
    // order ranks by how badly a service is doing, which says nothing about a set that
    // is all going down together.
    let going: Vec<String> = running
        .iter()
        .filter(|service| service.state.stoppable())
        .filter(|service| !holds.contains(&service.id))
        .map(|service| service.id.clone())
        .collect();
    let stopped = stopping_order(running, &going);

    Switched {
        stopped,
        started,
        kept,
        stop_command: None,
    }
}

/// Whether the stack says this one belongs to the operating system.
fn host_managed(running: &[Service], id: &str) -> bool {
    running
        .iter()
        .any(|service| service.id == id && service.state == State::HostManaged)
}

/// A plan naming only what a switch is leaving behind.
///
/// Compose can be handed a service name only when that service's profile is active,
/// so stopping what falls outside the new closure means naming the profiles being
/// *left* rather than the ones being arrived at. Naming the new closure's profiles
/// here would produce a command that quietly stopped nothing.
fn leaving(manifest: &Manifest, stopping: &[String]) -> Plan {
    Plan {
        forms: Vec::new(),
        profiles: manifest
            .services
            .iter()
            .filter(|service| stopping.contains(&service.id))
            .map(|service| service.profile.clone())
            .collect(),
        services: stopping.to_vec(),
        dropped: Vec::new(),
        filtered: Vec::new(),
        footprint: crate::stack::closure::Footprint::default(),
    }
}

#[cfg(test)]
mod tests;
