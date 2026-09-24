//! What a surface saw, turned into the findings a summary is computed from.
//!
//! A surface reads containers and a tunnel; the summary reads conditions. This is
//! the one place that translation happens, so every surface agrees on what counts
//! as wrong and how much it matters — and so "the torrent client's traffic is not
//! behind the tunnel is critical" is written down once rather than at each caller.
//!
//! How much a service's failure matters comes from what the manifest says its
//! absence costs, not from the fact that a container is down. A failed subtitle
//! fetcher and a failed download client are not the same event, and grading them
//! alike is how an operator learns to ignore the summary.
//!
//! Every check looked at is reported, including the ones that found nothing. The
//! condition store needs both: something wrong raises, nothing wrong clears, and a
//! check that could not be run says nothing at all — so a fault is never forgotten
//! merely because nobody looked this time.

use lemonfiber_manifest::Criticality;

use crate::condition::Fault;
use crate::docker::{Service, State};
use crate::error::Severity;

/// The check the tunnel's egress is filed under.
pub(crate) const EGRESS_CHECK: &str = "vpn.egress";

// The kinds of event this module raises. Named here rather than spelled at each
// site, since a kind is what an operator switches off and what groups four
// services failing alike into one alert.

/// The download client's traffic was proven to leave outside the tunnel.
pub const LEAKING: &str = "vpn.egress.leaking";
/// Whether it is behind the tunnel could not be established either way.
pub const UNVERIFIED: &str = "vpn.egress.unverified";
/// A service exited without being asked to.
pub(crate) const STOPPED: &str = "service.stopped";
/// A service is exiting and restarting repeatedly.
pub(crate) const CRASH_LOOPING: &str = "service.crash-looping";
/// A service is running and its own probe says it is not working.
pub const UNHEALTHY: &str = "service.unhealthy";

/// What the tunnel turned out to be doing, where a surface looked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Egress {
    /// This stack has no VPN-contained client, so there is nothing to be wrong.
    NotApplicable,
    /// The client's traffic was proven to leave through the tunnel.
    Behind,
    /// The client's traffic was proven to leave somewhere else.
    Leaking,
    /// It could not be established either way.
    Unreadable,
}

/// Every check this surface ran, and what it found.
///
/// `None` against a check means it ran and found nothing — which clears a standing
/// condition. A check that could not be run does not appear at all, because "I
/// could not tell" must never be recorded as "it is fine".
#[must_use]
pub fn observed(services: &[Service], egress: Egress) -> Vec<(String, Option<Fault>)> {
    let mut looked: Vec<(String, Option<Fault>)> = Vec::new();
    if egress != Egress::NotApplicable {
        looked.push((EGRESS_CHECK.to_owned(), tunnel(egress, services)));
    }
    looked.extend(
        services
            .iter()
            // Host-managed services are not lemonfiber's to start, so reporting one
            // as broken would blame it for something outside its control.
            .filter(|service| service.state != State::HostManaged)
            .map(|service| (check_of(service), service_fault(service, services))),
    );
    looked
}

/// The check one service is filed under.
fn check_of(service: &Service) -> String {
    format!("service.{}", service.id)
}

/// What the tunnel amounts to.
///
/// A leak is critical — its consequence is outside the machine and cannot be
/// undone by stopping the stack afterwards. A tunnel that could not be read is a
/// warning rather than nothing: the reason to run a torrent client behind a VPN is
/// unverified, and reporting silence as safety is the failure this whole feature
/// exists to prevent.
fn tunnel(egress: Egress, services: &[Service]) -> Option<Fault> {
    let fault = match egress {
        Egress::NotApplicable | Egress::Behind => return None,
        Egress::Leaking => Fault::new(
            LEAKING,
            Severity::Critical,
            "the download client's traffic is not going through the tunnel",
            "every peer it talks to can see this connection's own address, and stopping the stack \
             afterwards does not take that back",
            "stop the download client until the tunnel is proven to carry its traffic",
        )
        .or_else("check the gateway container is running and connected"),
        Egress::Unreadable => Fault::new(
            UNVERIFIED,
            Severity::Warning,
            "whether the download client is behind the tunnel could not be established",
            "the reason for running it behind a tunnel is unproven, so its traffic is best treated \
             as exposed until it is",
            "check the gateway container is running",
        )
        .or_else("set an IP-echo address so the egress can be compared"),
    };
    // A gateway that is itself down is why the tunnel cannot be trusted, rather
    // than a second thing wrong beside it.
    match services
        .iter()
        .find(|service| service.state.wants_attention() && is_gateway(service))
    {
        Some(gateway) => Some(fault.caused_by(&check_of(gateway))),
        None => Some(fault),
    }
}

/// Whether a service is what other services route their traffic through — the one
/// whose failure takes the tunnel with it.
fn is_gateway(service: &Service) -> bool {
    service.depends_on.is_empty() && service.criticality == Criticality::Critical
}

/// What is wrong with one service, or nothing where it is fine.
fn service_fault(service: &Service, services: &[Service]) -> Option<Fault> {
    if !service.state.wants_attention() {
        return None;
    }
    let fault = Fault::new(
        kind_of(service),
        severity_of(service.criticality),
        &summary_of(service),
        &meaning_of(service),
        &remedy_of(service),
    )
    .or_else("read its logs for what it said before it stopped");

    // A service that cannot start because something it depends on is down is one
    // problem with the thing underneath, not two independent failures.
    match service.depends_on.iter().find(|needed| {
        services
            .iter()
            .any(|other| &&other.id == needed && other.state.wants_attention())
    }) {
        Some(needed) => Some(fault.caused_by(&format!("service.{needed}"))),
        None => Some(fault),
    }
}

/// Which kind of failure this is — what four services failing the same way have
/// in common, and what an operator switches off.
const fn kind_of(service: &Service) -> &'static str {
    match service.state {
        State::CrashLooping => CRASH_LOOPING,
        State::Unhealthy => UNHEALTHY,
        _ => STOPPED,
    }
}

/// How much a service's failure matters, from what its absence costs.
///
/// The two least costly rungs are advisory: a failure confined to services that
/// only make things better is worth knowing and not worth waking anyone for.
const fn severity_of(criticality: Criticality) -> Severity {
    match criticality {
        Criticality::Critical => Severity::Critical,
        Criticality::Core => Severity::Error,
        Criticality::Important => Severity::Warning,
        Criticality::Enhancing | Criticality::Optional => Severity::Advisory,
    }
}

/// What is wrong with one service, in the operator's words rather than the
/// engine's — the same state renders differently depending on how it got there.
fn summary_of(service: &Service) -> String {
    let name = &service.name;
    match service.state {
        State::CrashLooping => format!("{name} keeps restarting"),
        State::Unhealthy => format!("{name} is running but its own check is failing"),
        _ => match service.exit {
            Some(code) => format!("{name} stopped on its own (exit {code})"),
            None => format!("{name} stopped on its own"),
        },
    }
}

/// What one service's absence costs, from how much the form depends on it.
///
/// Read off the criticality the manifest already declares rather than guessed per
/// service: the manifest is where "how much does this matter" is decided, and a
/// second opinion here would be a second thing to keep in step with it.
fn meaning_of(service: &Service) -> String {
    let name = &service.name;
    match service.criticality {
        Criticality::Critical => {
            format!("what depends on {name} is not safe to keep running while it is down")
        }
        Criticality::Core => format!("the stack does not do what it is for without {name}"),
        Criticality::Important => {
            format!("the rest carries on; the part {name} does is not happening")
        }
        Criticality::Enhancing => {
            format!("nothing essential stops — {name} is what makes the rest nicer")
        }
        Criticality::Optional => format!("nothing depends on {name}; the stack is unaffected"),
    }
}

/// The first thing to try, which differs by how the service failed: restarting
/// something that is already restarting itself achieves nothing.
fn remedy_of(service: &Service) -> String {
    let name = &service.name;
    match service.state {
        State::CrashLooping => {
            format!("{name} is restarting itself and failing; its configuration is the usual cause")
        }
        State::Unhealthy => format!("give {name} a moment, then restart it if it does not settle"),
        _ => format!("start {name} again"),
    }
}

#[cfg(test)]
mod tests;
