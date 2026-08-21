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

use super::{compose, settled_into, Composed};
use crate::app::{Ctx, Outcome};
use crate::docker::{survey, Service, State};
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
pub(crate) async fn switch(ctx: &Ctx, forms: &[String]) -> Result<Outcome, Box<Problem>> {
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
        .engine
        .list(&ctx.settings.project)
        .await
        .map_err(|err| Box::new(err.problem()))?;
    let running = survey(&manifest, &declared, &containers);

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

    let mut report = LifecycleReport {
        action: SWITCH.to_owned(),
        plan,
        command: command.clone(),
        rehearsed: ctx.dry_run,
        status: None,
        services: Vec::new(),
        condition: None,
        stack_edits,
        forwarding: None,
        switched: Some(switched),
    };

    if ctx.dry_run {
        return Ok(Outcome::Lifecycle(report));
    }

    if let Some(stopping) = stopping {
        let output = ctx
            .runner
            .run(&stopping)
            .await
            .map_err(|err| Box::new(err.problem()))?;
        report.status = output.status;
        if !output.succeeded() {
            return Ok(Outcome::Lifecycle(report));
        }
    }

    let started = ctx
        .runner
        .run(&command)
        .await
        .map_err(|err| Box::new(err.problem()))?;
    report.status = started.status;
    if !started.succeeded() {
        return Ok(Outcome::Lifecycle(report));
    }

    // Waited for last rather than inside a branch, because what a switch started is
    // the same thing bringing a form up starts, and it is owed the same wait.
    settled_into(ctx, &manifest, &mut report).await?;
    Ok(Outcome::Lifecycle(report))
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

    // Sorted, because what is being stopped is not in the closure and so has no
    // declared order to borrow. The survey's own order ranks by how badly a service
    // is doing, which says nothing about a set that is all going down together.
    let mut stopped: Vec<String> = running
        .iter()
        .filter(|service| service.state.stoppable())
        .filter(|service| !holds.contains(&service.id))
        .map(|service| service.id.clone())
        .collect();
    stopped.sort();

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
    }
}

#[cfg(test)]
mod tests {
    use super::{leaving, moved};
    use crate::docker::{Criticality, Service, State};
    use lemonfiber_manifest::Manifest;

    const STACK: &str = include_str!("../../../../../assets/media-stack/stack.toml");

    fn service(id: &str, state: State) -> Service {
        Service {
            id: id.to_owned(),
            name: id.to_owned(),
            profile: "media".to_owned(),
            state,
            criticality: Criticality::Core,
            depends_on: Vec::new(),
            exit: None,
        }
    }

    /// The whole point of the verb: a service both shapes hold is not touched.
    #[test]
    fn a_service_the_new_shape_also_holds_is_kept_rather_than_restarted() {
        let running = vec![
            service("jellyfin", State::Healthy),
            service("qbittorrent", State::Healthy),
        ];
        let holds = vec!["jellyfin".to_owned(), "sonarr".to_owned()];

        let switched = moved(&running, &holds);

        assert_eq!(switched.kept, vec!["jellyfin".to_owned()]);
        assert_eq!(
            switched.stopped,
            vec!["qbittorrent".to_owned()],
            "only what fell outside the new closure is stopped"
        );
        assert_eq!(
            switched.started,
            vec!["sonarr".to_owned()],
            "and only what the new closure adds is started"
        );
    }

    #[test]
    fn nothing_running_means_a_switch_starts_everything_and_stops_nothing() {
        let switched = moved(&[], &["sonarr".to_owned(), "radarr".to_owned()]);
        assert!(switched.stopped.is_empty() && switched.kept.is_empty());
        assert_eq!(
            switched.started,
            vec!["sonarr".to_owned(), "radarr".to_owned()]
        );
    }

    /// A container that exists and is not running has nothing to stop, so naming it
    /// would produce a Compose command that did nothing and a report that lied.
    #[test]
    fn a_service_that_is_not_up_is_not_something_to_stop() {
        let running = vec![
            service("prowlarr", State::Stopped),
            service("bazarr", State::Absent),
            service("flaresolverr", State::Failed),
        ];
        let switched = moved(&running, &[]);
        assert!(
            switched.stopped.is_empty(),
            "{:?} is stopped, absent or already exited",
            switched.stopped
        );
    }

    /// Crash-looping is the case that most wants stopping — it is up, repeatedly.
    #[test]
    fn a_crash_looping_service_outside_the_new_shape_is_stopped() {
        let running = vec![service("sabnzbd", State::CrashLooping)];
        assert_eq!(moved(&running, &[]).stopped, vec!["sabnzbd".to_owned()]);
    }

    /// The operating system owns a native Jellyfin, so a switch neither starts it
    /// nor claims to have.
    #[test]
    fn a_host_managed_service_is_neither_started_nor_stopped() {
        let running = vec![service("jellyfin", State::HostManaged)];
        let switched = moved(&running, &["jellyfin".to_owned()]);
        assert!(
            switched.started.is_empty() && switched.stopped.is_empty() && switched.kept.is_empty(),
            "{switched:?}"
        );
    }

    /// The subtle one: Compose will not accept a service name whose profile is not
    /// active, so the stop command has to name the profiles being left.
    #[test]
    fn the_stop_names_the_profiles_it_is_leaving_not_the_ones_it_is_going_to() {
        let plan = Manifest::from_toml(STACK)
            .ok()
            .map(|manifest| leaving(&manifest, &["qbittorrent".to_owned()]));
        assert!(
            plan.as_ref()
                .is_some_and(|plan| plan.profiles.iter().any(|profile| profile == "torrent")),
            "{plan:?}"
        );
    }
}
