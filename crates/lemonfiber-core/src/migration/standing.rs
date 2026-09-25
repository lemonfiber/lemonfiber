//! What is standing on this machine now, and what of it collides with lemonfiber.
//!
//! Grouping, and the two readings that fall out of it: which ports lemonfiber would
//! want that something else already answers on, and which services of a recognisable
//! stack it could not take over.

use std::collections::{BTreeMap, BTreeSet};

use crate::model::{ConflictReport, OccupantReport, StandingReport, UnsupportedReport};
use crate::ports::docker::{Container, Lifecycle};

use super::Ours;

/// Every project on this machine that is not lemonfiber's, by project name.
#[must_use]
pub fn here(project: &str, seen: &[Container], ours: &[Ours]) -> Vec<StandingReport> {
    let mut projects: BTreeMap<&str, Vec<OccupantReport>> = BTreeMap::new();
    for container in seen.iter().filter(|found| found.project != project) {
        projects
            .entry(&container.project)
            .or_default()
            .push(occupant(container, ours));
    }

    projects
        .into_iter()
        .map(|(project, mut services)| {
            services.sort_by(|one, two| one.service.cmp(&two.service));
            StandingReport {
                project: project.to_owned(),
                services,
            }
        })
        .collect()
}

/// One existing container, as it appears in the survey.
fn occupant(container: &Container, ours: &[Ours]) -> OccupantReport {
    let mut ports: Vec<u16> = container
        .published
        .iter()
        .map(|published| published.port)
        .collect();
    ports.sort_unstable();
    ports.dedup();

    OccupantReport {
        service: container.service.clone(),
        running: container.lifecycle == Lifecycle::Running,
        ports,
        adoptable: ours.iter().any(|one| one.service == container.service),
    }
}

/// Every host port anything already standing here answers on.
#[must_use]
pub fn taken(standing: &[StandingReport]) -> BTreeSet<u16> {
    standing
        .iter()
        .flat_map(|project| project.services.iter())
        .flat_map(|service| service.ports.iter().copied())
        .collect()
}

/// Every port lemonfiber wants that an existing service already answers on.
///
/// Reported before a plan rather than discovered while applying one, because a port
/// already taken is the ordinary way a second stack fails to start, and finding it then
/// leaves the operator with two half-running setups.
#[must_use]
pub fn conflicts(ours: &[Ours], standing: &[StandingReport]) -> Vec<ConflictReport> {
    let mut found: Vec<ConflictReport> = ours
        .iter()
        .filter_map(|one| one.port.map(|port| (one, port)))
        .flat_map(|(one, port)| {
            standing.iter().flat_map(move |project| {
                project
                    .services
                    .iter()
                    .filter(move |occupant| occupant.ports.contains(&port))
                    .map(move |occupant| ConflictReport {
                        port,
                        wanted_by: one.service.clone(),
                        held_by: format!("{}/{}", project.project, occupant.service),
                    })
            })
        })
        .collect();
    found.sort_by(|one, two| one.port.cmp(&two.port).then(one.held_by.cmp(&two.held_by)));
    found
}

/// Every service of a recognisable setup that lemonfiber could not take over.
///
/// Only of a project already holding at least one service lemonfiber runs. A project
/// with none of them is somebody's unrelated work rather than a stack being migrated,
/// and naming its every service unsupported would say lemonfiber had weighed adopting a
/// database it was never asked about. Such a project is still reported as standing here,
/// because the ports it holds are just as taken either way.
#[must_use]
pub fn unsupported(standing: &[StandingReport]) -> Vec<UnsupportedReport> {
    standing
        .iter()
        .filter(|project| project.services.iter().any(|service| service.adoptable))
        .flat_map(|project| {
            project
                .services
                .iter()
                .filter(|service| !service.adoptable)
                .map(move |service| UnsupportedReport {
                    what: format!("{}/{}", project.project, service.service),
                    because: "lemonfiber does not run this service, so it would be left \
                              exactly as it is rather than taken over"
                        .to_owned(),
                })
        })
        .collect()
}

#[cfg(test)]
mod tests;
