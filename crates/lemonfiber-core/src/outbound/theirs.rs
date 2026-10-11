//! The requests the stack's own services make, attributed to them.
//!
//! An indexer query is the indexer aggregator asking an indexer. A poster is a curator
//! asking a metadata provider. A peer connection is the torrent client being a torrent
//! client. Counting any of those as lemonfiber's would overstate what this product does —
//! and leaving them out would understate what running the stack does, which is the thing an
//! operator is actually deciding about.
//!
//! **The stack is what says so.** A service declares where it reaches and what it
//! asks for in its own manifest entry, and that is what the inventory carries. So
//! adding a service to a Compose project is a change to that project and to nothing
//! here, and the prose describing nineteen services versions with the nineteen
//! services rather than with this binary.
//!
//! It was not always so, and what it replaced is worth stating, because the thing it
//! was defending is real. This file used to carry the prose itself — a table keyed by
//! service id, held against the stack by a set-equality test in both directions, so a
//! service arriving in the stack turned this crate red until somebody edited Rust.
//! That is the one thing adding a service to a Compose project must never cost. But
//! an inventory of what leaves a machine is only honest if a service cannot arrive in
//! it unlisted, and that is what the test bought.
//!
//! Both, now, and from the stack alone. A service the stack describes is described. A
//! service nothing describes is carried into the inventory saying lemonfiber has no
//! record of what it reaches, which is the truth and is neither a guess nor a silent
//! omission. And the shipped stack is held to describing all of its own: the test
//! below fails the build rather than the operator, because the pairing of this binary
//! with the stack it embeds is decided here and not by somebody's machine.
//!
//! What a second copy here would cost is not hypothetical. The table outlived its
//! last reader by one release and had already drifted from the manifest in the one
//! entry nobody happened to reread — which is what a duplicate does when only one of
//! the two is the one anything consults.

use super::Elsewhere;
use lemonfiber_manifest::Service;

/// Where an unrecorded service is said to reach, which is the one thing that can
/// honestly be said about it.
///
/// A sentence rather than an empty string, because empty already means *nothing
/// leaves this machine* in this report and the two are opposite claims.
const UNKNOWN: &str = "not known to lemonfiber";

/// What is said about a destination a plugin's recipes declare.
const CARRIED: &str = "A destination this plugin's recipes declare they may call, or carry \
                       a value it captured to. Declared in its manifest and held to it: a \
                       recipe reaching anywhere else is refused before it is installed.";

/// What is said about a service this build ships no record for.
const NO_RECORD: &str = "This service is not one lemonfiber knows, so nothing here can say \
                         where it reaches or what it asks for. It is listed because leaving \
                         it out would make this inventory read as complete while it was \
                         short. Its own documentation, and the Compose file that declares \
                         it, are what answer this.";

/// What the services in this stack reach, in the order the stack declares them.
///
/// Every service declared, whether or not it describes itself. One that does not is
/// carried anyway and reported as undescribed: never dropped, and never rendered as
/// reaching nothing.
pub(super) fn elsewhere(services: &[Service]) -> Vec<Elsewhere> {
    services
        .iter()
        .map(|service| {
            said(
                &service.id,
                service.reaches.as_deref(),
                service.asks_for.as_deref(),
                crate::origin::Origin::Bundled,
            )
        })
        .collect()
}

/// What a service says about itself, or the admission that nothing describes it.
///
/// Both halves or neither, which the manifest's own validation holds it to: half an
/// answer here would attribute a purpose to a service the same report says goes
/// nowhere, so it is treated as the silence it is. A stack service's own entry is the
/// only source for it, because it is the one that travels with the stack — a fork
/// describes its own services, and a service added to the stack needs no release of
/// this binary to be described.
///
/// The admission is reached by a service an operator's own stack declares and says
/// nothing about. For the stack this binary embeds it is reached by nothing at all,
/// which is a property of that stack rather than of this function, and the test below
/// is what keeps it one.
fn said(
    service: &str,
    reaches: Option<&str>,
    asks_for: Option<&str>,
    origin: crate::origin::Origin,
) -> Elsewhere {
    let (destination, purpose, recorded) = match (reaches, asks_for) {
        (Some(destination), Some(purpose)) => (destination, purpose, true),
        _ => (UNKNOWN, NO_RECORD, false),
    };
    Elsewhere {
        service: service.to_owned(),
        destination: destination.to_owned(),
        purpose: purpose.to_owned(),
        recorded,
        origin,
    }
}

/// What installed plugins bring to this account: each of their services, and every
/// destination outside the stack their recipes declare.
///
/// A plugin's service is listed as it says where it reaches and what it asks for, and
/// one that says nothing as one lemonfiber has no record of, attributed to its plugin,
/// for the reason an undescribed bundled service is listed: leaving it out would make
/// the account read as complete while it was short. A destination a recipe
/// declares is recorded, because the plugin declared it and was held to it, and it is
/// listed only where it is outside the stack: a recipe reaching one of the stack's own
/// services is not something leaving the machine.
pub(super) fn brought(stack: &[Service], installed: &[crate::plugin::Installed]) -> Vec<Elsewhere> {
    let inside = |to: &str| stack.iter().any(|service| service.id == to);
    installed
        .iter()
        .flat_map(|one| {
            let origin = crate::origin::Origin::Plugin {
                named: one.plugin.clone(),
            };
            let services = one.services.iter().map({
                let origin = origin.clone();
                move |placed| {
                    said(
                        &placed.service,
                        placed.reaches.as_deref(),
                        placed.asks_for.as_deref(),
                        origin.clone(),
                    )
                }
            });
            let hosts = one
                .declared
                .reaches
                .iter()
                .filter(|to| !inside(to))
                .map(move |to| Elsewhere {
                    service: one.plugin.clone(),
                    destination: to.clone(),
                    purpose: CARRIED.to_owned(),
                    recorded: true,
                    origin: origin.clone(),
                });
            services.chain(hosts).collect::<Vec<Elsewhere>>()
        })
        .collect()
}

#[cfg(test)]
mod tests;
