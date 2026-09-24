//! What is already answering on the ports this stack is about to want.
//!
//! Two stacks on one host is a supported arrangement — distinct project names, and
//! ports that do not overlap — and the second half of that is the one nobody can
//! check by reading their own configuration. The comparison that finds an overlap
//! has existed for as long as the migration survey has, and the survey was its only
//! caller: an operator who never ran `migrate` met the same overlap as a Compose
//! error naming a port and nothing else, part-way through a start that had already
//! brought half the stack up.
//!
//! So it is asked before a start as well. Asked, not enforced: what is on this
//! machine is the operator's, a port they deliberately share is their business, and
//! refusing to start over it would make this the thing standing between them and
//! their stack. The start goes ahead and the report says what it found, which is the
//! difference between an operator who knows why the bind failed and one who does not.
//!
//! Two questions, because a port can be held by something that is not a container at
//! all: a media server the operator installed themselves, a service the system
//! started at boot, a development server somebody left running. The engine cannot see
//! any of them, so the ports it cannot account for are put to the machine itself.
//!
//! Nothing here refuses to answer either. An engine that will not say which projects
//! it holds leaves an empty list rather than a failure, and a machine that will not
//! say what is listening leaves the same, because a start that cannot be pre-flighted
//! is still a start the operator asked for.

use std::collections::BTreeSet;

use lemonfiber_manifest::Manifest;

use crate::migration::{pins, standing, Ours};
use crate::model::ConflictReport;
use crate::ports::docker::{Container, Image};

use super::Ctx;

/// Every host port the services in this plan would publish that another Compose
/// project on this machine already answers on.
///
/// Narrowed to the plan rather than to the whole manifest, because a conflict on a
/// service that is not being started is not a conflict with anything: an operator
/// running only the television form has no quarrel with whoever holds the music
/// service's port.
///
/// lemonfiber's own project is excluded by the comparison itself, so bringing this
/// stack up a second time reports nothing — it is the one project whose ports are
/// meant to be these.
///
/// Nothing is asked of the engine where the answer is already settled: a plan holding
/// no service that publishes a port has nothing to clash over, and that is the common
/// case for everything except a start.
pub(crate) async fn conflicting_ports(
    ctx: &Ctx,
    manifest: &Manifest,
    starting: &[String],
) -> Vec<ConflictReport> {
    // What this start would want, worked out before anything is asked of the engine.
    // A plan that publishes nothing cannot clash with anything, and a question to a
    // daemon whose answer is decided in advance is a question worth not asking — a
    // teardown, a stack of services that bind nothing, and a machine with no engine
    // reachable at all all come out here.
    let ours: Vec<Ours> = pins(manifest)
        .into_iter()
        .filter(|one| one.port.is_some())
        .filter(|one| starting.iter().any(|service| service == &one.service))
        .collect();
    if ours.is_empty() {
        return Vec::new();
    }

    let Ok(images) = ctx.images.images().await else {
        return Vec::new();
    };
    let Some(seen) = every_container(ctx, &images).await else {
        return Vec::new();
    };
    let standing = standing::here(&ctx.settings.project, &seen, &ours);
    let mut found = standing::conflicts(&ours, &standing);
    found.extend(outside_the_engine(ctx, &ours, &seen).await);
    found
}

/// What a port nothing in the container engine explains is said to be held by.
///
/// Deliberately vague about who, because that is as much as can be honestly said: the
/// programs asked report a listener, and the operator knows what they run.
const OUTSIDE: &str = "something on this machine outside your container engine";

/// Every port this plan wants that something not in a container is already holding.
///
/// The comparison above reads the container engine, so the whole of what it can see
/// is containers — and the ordinary way a start fails to bind is a program the
/// operator installed themselves, sitting on a port a service here also wants. To
/// that comparison such a port is free, and the first anybody hears of it is Compose
/// failing part-way through a start.
///
/// Only ports no container explains are asked about, and every container on this
/// machine counts — lemonfiber's own included. Our containers publish exactly these
/// ports while the stack is up, and a machine is perfectly able to see them
/// listening: without that exclusion, starting a stack that is already running would
/// report every one of its own ports as held by a stranger.
async fn outside_the_engine(ctx: &Ctx, ours: &[Ours], seen: &[Container]) -> Vec<ConflictReport> {
    let explained: BTreeSet<u16> = seen
        .iter()
        .flat_map(|container| container.published.iter())
        .map(|published| published.port)
        .collect();
    let unaccounted: Vec<u16> = ours
        .iter()
        .filter_map(|one| one.port)
        .filter(|port| !explained.contains(port))
        .collect();

    let held = ctx.site.answering_on(&unaccounted).await;
    let mut found: Vec<ConflictReport> = ours
        .iter()
        .filter_map(|one| one.port.map(|port| (one, port)))
        .filter(|(_, port)| held.contains(port))
        .map(|(one, port)| ConflictReport {
            port,
            wanted_by: one.service.clone(),
            held_by: OUTSIDE.to_owned(),
        })
        .collect();
    found.sort_by(|one, two| {
        one.port
            .cmp(&two.port)
            .then(one.wanted_by.cmp(&two.wanted_by))
    });
    found
}

/// Every container of every Compose project this engine knows, or nothing where it
/// would not say.
///
/// The project names come from the images the engine holds, because there is no ask
/// for "every project" — a project is a label on a container, and the images are what
/// carries the set of them without listing every container on the machine first.
///
/// `None` where any part of the walk failed, and the distinction is load-bearing for
/// the survey that shares this: that nothing was found and that nothing could be
/// looked at are different facts, and only one of them makes it safe to stand a stack
/// up here.
pub(crate) async fn every_container(ctx: &Ctx, images: &[Image]) -> Option<Vec<Container>> {
    let mut projects: BTreeSet<&str> = BTreeSet::new();
    for image in images {
        projects.extend(
            image
                .projects
                .iter()
                .map(String::as_str)
                .filter(|project| !project.is_empty()),
        );
    }

    let mut seen: Vec<Container> = Vec::new();
    for project in projects {
        seen.extend(ctx.engine.list(project).await.ok()?);
    }
    Some(seen)
}

#[cfg(test)]
mod tests;
