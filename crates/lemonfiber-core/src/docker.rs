//! Making sense of what the engine reports.
//!
//! The port returns containers; this module turns them into the state a surface
//! renders — correlating containers back to the services that declared them,
//! and deciding what "started" actually means.
//!
//! Nothing here talks to anything. It is a function of a manifest, a plan and a
//! listing, which is what lets every state a service can be in be exercised
//! without a daemon, an image, or a running stack.
//!
//! The vocabulary is deliberately larger than the engine's. "Up" is an
//! ambiguity: a container whose process exists tells you nothing about whether
//! the application inside it is answering, and an operator told "started" who
//! then gets connection refused has been lied to.

use std::collections::{BTreeMap, BTreeSet};

use lemonfiber_manifest::Manifest;

// `Service` exposes a criticality as a public field, so the type has to be nameable by
// whoever reads one. It is the manifest's, re-exported here rather than left reachable
// only through a crate a consumer may not depend on.
pub use lemonfiber_manifest::Criticality;
use serde::Serialize;

use crate::ports::docker::{Container, Health, Lifecycle};

/// What one service is actually doing.
///
/// Ordered from worst to best, so a form's condition is the minimum across its
/// services and needs no comparison table. The declaration order is therefore
/// load-bearing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "ServiceState")]
pub enum State {
    /// Exited without being asked to, and is not coming back on its own.
    Failed,
    /// Exiting and restarting repeatedly.
    CrashLooping,
    /// Running, and its own probe says it is not working.
    Unhealthy,
    /// No container exists.
    Absent,
    /// A container exists and is not running.
    Stopped,
    /// Running, and still inside its probe's start period.
    Starting,
    /// Running, and declaring no probe by which to know more.
    ///
    /// Distinct from healthy on purpose. A service that cannot be asked is not
    /// a service that answered, and rendering the two identically is how a
    /// dashboard comes to be believed about something it never checked.
    Running,
    /// Running and passing its own probe.
    Healthy,
    /// The operating system owns this one, not lemonfiber.
    HostManaged,
}

impl State {
    /// Whether this state is one that starting up is still waiting on.
    ///
    /// Settled does not mean well. A service that is crash-looping has settled
    /// into crash-looping, and waiting longer will not improve it.
    #[must_use]
    pub const fn settled(self) -> bool {
        !matches!(self, Self::Starting | Self::Absent | Self::Stopped)
    }

    /// Whether this state is one an operator needs to do something about.
    #[must_use]
    pub(crate) const fn wants_attention(self) -> bool {
        matches!(self, Self::Failed | Self::CrashLooping | Self::Unhealthy)
    }

    /// Whether lemonfiber has anything here to stop.
    ///
    /// `Absent` and `Stopped` have nothing to stop, and `Failed` has already
    /// stopped itself. A crash-looping service does have something to stop, and
    /// is the case that most wants stopping. `HostManaged` has something running
    /// too, but it is the operating system's and not lemonfiber's — which is the
    /// whole of what leaving a native Jellyfin alone comes to.
    #[must_use]
    pub(crate) const fn stoppable(self) -> bool {
        !matches!(
            self,
            Self::Absent | Self::Stopped | Self::Failed | Self::HostManaged
        )
    }
}

/// One service, as it stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Service {
    /// The service's identifier, which is also its Compose service name.
    pub id: String,
    /// What it is called in front of an operator.
    pub name: String,
    /// What it does for the operator, in the stack's own words.
    ///
    /// Carried on the service rather than looked up where it is shown, because this
    /// is the one struct every surface reads: a listing, the machine-readable reply,
    /// the web API and the terminal's panel all render this, and a description
    /// fetched separately by each of them would be four chances to render three.
    ///
    /// The stack's words rather than lemonfiber's, for the reason its absence cost is:
    /// a stack that adds a service should not need a lemonfiber release before it can
    /// say what that service is for.
    pub describes: String,
    /// The profile that declared it.
    pub profile: String,
    /// Every form it is running for, in the order the stack declares them.
    ///
    /// All of them rather than one, because a service two forms share is there for
    /// both, and stopping one of them leaves it running for the other. Empty where no
    /// form it belongs to is up: a service nobody's form holds is not missing from one.
    pub forms: Vec<String>,
    /// What it is doing.
    pub state: State,
    /// How much its absence costs, so a summary can weigh it.
    pub criticality: Criticality,
    /// The services it needs before it can work, as the manifest declares them.
    /// Carried so a failure can be attributed to the thing underneath it rather
    /// than counted as one more independent thing wrong.
    pub depends_on: Vec<String>,
    /// How it exited, where it has exited.
    pub exit: Option<i32>,
}

/// What a whole set of services amounts to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Condition {
    /// Nothing is running.
    Inactive,
    /// Running, with at least one service in a state that wants attention.
    Degraded,
    /// Some services are up and others are not.
    Partial,
    /// Everything expected is up and answering.
    Active,
}

/// Read one container's state, given whether the service declares a probe.
///
/// A container reaching `running` says nothing about whether the application
/// inside it has finished starting, which is why a probe's verdict outranks the
/// process existing. Where there is no probe, `Running` is the strongest claim
/// available and is reported as exactly that.
fn read(container: &Container) -> State {
    match container.lifecycle {
        Lifecycle::Running => match container.health {
            Health::Starting => State::Starting,
            Health::Healthy => State::Healthy,
            Health::Unhealthy => State::Unhealthy,
            Health::None => State::Running,
        },
        // Restarting is the engine's word for both a deliberate restart and a
        // container failing on a loop. Told apart by nothing else the engine
        // offers, the safer reading is the one that shows the operator
        // something rather than the one that hides it behind `starting`.
        Lifecycle::Restarting => State::CrashLooping,
        // Created has never run; paused exists and is not serving. Both are a
        // container that is there and doing nothing, which is what stopped
        // means — a paused container's last health verdict is a claim about a
        // container that can no longer answer.
        Lifecycle::Created | Lifecycle::Paused => State::Stopped,
        // A clean exit is a service that was stopped; any other is one that
        // fell over. An engine that has forgotten the code is not evidence of
        // either, so it is reported as merely stopped.
        Lifecycle::Exited | Lifecycle::Dead | Lifecycle::Removing => match container.exit {
            Some(0) | None => State::Stopped,
            Some(_) => State::Failed,
        },
    }
}

/// What every service in the named profiles is doing, and which forms it is up for.
///
/// Services are taken from the manifest rather than from the listing, so a
/// service that was never started is reported as absent rather than omitted —
/// which is the difference between a dashboard saying nothing is wrong and one
/// saying nothing is there.
///
/// Which forms a service is up for is read from the whole stack whatever profiles
/// were asked about, since whether a form is up is a question about all of its
/// services. `protocols` decides what each form resolves to.
#[must_use]
pub fn survey(
    manifest: &Manifest,
    profiles: &[String],
    containers: &[Container],
    protocols: crate::config::Protocols,
) -> Vec<Service> {
    let mut whole = surveyed(manifest, containers);
    let brought = crate::stack::standing::brought(manifest, protocols, &whole);
    whole.retain(|service| profiles.iter().any(|profile| profile == &service.profile));
    for service in &mut whole {
        service.forms = brought
            .iter()
            .filter(|(_, plan)| plan.services.contains(&service.id))
            .map(|(form, _)| form.clone())
            .collect();
    }
    whole
}

/// What every service the stack declares is doing, worst first.
fn surveyed(manifest: &Manifest, containers: &[Container]) -> Vec<Service> {
    let found: BTreeMap<&str, &Container> = containers
        .iter()
        .map(|container| (container.service.as_str(), container))
        .collect();

    let mut services: Vec<Service> = manifest
        .services
        .iter()
        .map(|service| Service {
            id: service.id.clone(),
            name: service.name.clone(),
            describes: service.describes.clone(),
            profile: service.profile.clone(),
            forms: Vec::new(),
            state: if service.host_managed {
                State::HostManaged
            } else {
                found
                    .get(service.id.as_str())
                    .map_or(State::Absent, |c| read(c))
            },
            criticality: service.criticality,
            depends_on: service.depends_on.clone(),
            exit: found.get(service.id.as_str()).and_then(|c| c.exit),
        })
        .collect();

    // Worst first, then by name. An operator reading top to bottom meets the
    // thing that needs them before the nineteen things that do not.
    services.sort_by(|left, right| {
        left.state
            .cmp(&right.state)
            .then_with(|| left.id.cmp(&right.id))
    });
    services
}

/// What is said about a container the stack description has nothing to say about.
///
/// An unknown description rather than silence, and rather than a guess. Something is
/// running under this project that lemonfiber did not put there, and the two things
/// worth telling an operator about it are both in this sentence: it is here, and
/// nothing here knows what it does.
pub const UNDESCRIBED: &str = "not declared by this stack, so what it does is not recorded here";

/// A container running under this project that the stack description never declared.
///
/// Its own type rather than a [`Service`] with the fields left blank. A service
/// carries a profile and a criticality, and there is no honest value for either here:
/// filling them in would have lemonfiber asserting how much something matters when
/// the only thing it knows about it is that it exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Undeclared {
    /// The Compose service name the engine reports it under.
    pub id: String,
    /// What it is doing, read the same way a declared service's state is.
    pub state: State,
    /// What it does for the operator — which is exactly what is not known.
    pub describes: String,
}

/// The containers running under this project that the manifest never declared.
///
/// Reported rather than filtered out. [`survey`] walks the manifest, so anything the
/// manifest does not name is invisible to it — which is the right answer for every
/// question about what a form holds, and the wrong one for the question an operator
/// is asking when they look at a list of what is running. A container somebody added
/// to their own Compose override, or one left behind by a stack that has since
/// dropped the service, is part of what is on this machine under this project's name;
/// hiding it makes the listing a claim about the manifest rather than about the
/// machine.
///
/// Kept out of [`survey`] on purpose, and it is not tidiness. Everything that waits
/// for a stack to settle, decides what a form amounts to, or orders a stop reads that
/// function, and an undeclared container arriving in any of them would have lemonfiber
/// waiting on, grading, or stopping something that is not its own.
///
/// Ordered by name and reduced to one entry per name: two containers of one scaled
/// service are one thing the operator does not recognise, not two.
#[must_use]
pub fn undeclared(manifest: &Manifest, containers: &[Container]) -> Vec<Undeclared> {
    let declared: BTreeSet<&str> = manifest
        .services
        .iter()
        .map(|service| service.id.as_str())
        .collect();

    let strangers: BTreeMap<&str, State> = containers
        .iter()
        .filter(|container| !declared.contains(container.service.as_str()))
        .map(|container| (container.service.as_str(), read(container)))
        .collect();

    strangers
        .into_iter()
        .map(|(id, state)| Undeclared {
            id: id.to_owned(),
            state,
            describes: UNDESCRIBED.to_owned(),
        })
        .collect()
}

/// What a set of services amounts to, as one word.
#[must_use]
pub fn condition(services: &[Service]) -> Condition {
    // Host-managed services are not lemonfiber's to start, so counting them
    // would make a form permanently partial through no fault of the operator.
    let ours: Vec<&Service> = services
        .iter()
        .filter(|service| service.state != State::HostManaged)
        .collect();

    // Nothing is up when every service is either absent or stopped: a stack the
    // operator stopped is not "partly up", it is down with its containers kept.
    if ours.is_empty()
        || ours
            .iter()
            .all(|service| matches!(service.state, State::Absent | State::Stopped))
    {
        return Condition::Inactive;
    }
    if ours.iter().any(|service| service.state.wants_attention()) {
        return Condition::Degraded;
    }
    if ours
        .iter()
        .all(|service| matches!(service.state, State::Healthy | State::Running))
    {
        return Condition::Active;
    }
    Condition::Partial
}

/// The order to stop these in: whatever depends on a service goes down before it does.
///
/// A torrent client shares the tunnel's network namespace, so the moment the tunnel
/// stops the client has no network at all — mid-write, mid-announce, with peers it
/// cannot tell. Stopping the client first costs a few seconds and leaves it able to
/// shut down the way it knows how.
///
/// Read from each service's own `depends_on` rather than from a rule about VPNs. The
/// tunnel is the case that matters on this stack, but "stop a thing before the thing
/// it needs" is the general shape, and a stack that adds another such pair gets the
/// same treatment without lemonfiber learning about it.
///
/// Ties are broken by name so the same request always produces the same command.
///
/// Services that depend on each other in a circle cannot be ordered, and are emitted
/// as they stand: refusing to stop them would be worse than stopping them in an order
/// nobody can fault, since there is no correct one.
#[must_use]
pub(crate) fn stopping_order(running: &[Service], stopping: &[String]) -> Vec<String> {
    let depends = |dependent: &str, on: &str| {
        running
            .iter()
            .any(|service| service.id == dependent && service.depends_on.iter().any(|id| id == on))
    };

    let mut left: Vec<String> = stopping.to_vec();
    left.sort();
    let mut order = Vec::with_capacity(left.len());

    while !left.is_empty() {
        let (ready, waiting): (Vec<String>, Vec<String>) = left
            .iter()
            .cloned()
            .partition(|id| !left.iter().any(|other| depends(other, id)));

        if ready.is_empty() {
            // A circle. There is no order that satisfies it, so the remainder goes as
            // it stands rather than the whole stop being refused over it.
            order.extend(waiting);
            return order;
        }
        order.extend(ready);
        left = waiting;
    }
    order
}

/// The services that starting is still waiting on.
#[must_use]
pub fn unsettled(services: &[Service]) -> Vec<&Service> {
    services
        .iter()
        .filter(|service| service.state != State::HostManaged)
        .filter(|service| !service.state.settled())
        .collect()
}

#[cfg(test)]
mod tests;
