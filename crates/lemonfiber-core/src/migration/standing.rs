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
mod tests {
    use super::{conflicts, here, taken, unsupported};
    use crate::migration::tests::{container, ours, running};
    use crate::ports::docker::Lifecycle;

    #[test]
    fn our_own_project_is_not_somebody_elses_setup() {
        let seen = [container("lemonfiber", "sonarr", &[8989])];
        assert!(here("lemonfiber", &seen, &running()).is_empty());
    }

    #[test]
    fn an_existing_project_is_reported_with_the_ports_it_answers_on() {
        let seen = [container("media", "sonarr", &[8989])];
        let found = here("lemonfiber", &seen, &running());
        let ports = found
            .first()
            .and_then(|project| project.services.first())
            .map(|service| service.ports.clone());
        assert_eq!(ports, Some(vec![8989]), "{found:?}");
    }

    #[test]
    fn a_stopped_container_is_still_found_and_says_it_is_not_running() {
        let mut stopped = container("media", "sonarr", &[8989]);
        stopped.lifecycle = Lifecycle::Exited;
        let found = here("lemonfiber", &[stopped], &running());
        let is = found
            .first()
            .and_then(|project| project.services.first())
            .map(|service| service.running);
        assert_eq!(is, Some(false), "{found:?}");
    }

    #[test]
    fn a_repeated_port_is_reported_once() {
        let seen = [container("media", "sonarr", &[8989, 8989])];
        let found = here("lemonfiber", &seen, &running());
        let ports = found
            .first()
            .and_then(|project| project.services.first())
            .map(|service| service.ports.clone());
        assert_eq!(ports, Some(vec![8989]), "{found:?}");
    }

    #[test]
    fn services_of_one_project_read_in_a_settled_order() {
        let seen = [
            container("media", "sonarr", &[8989]),
            container("media", "radarr", &[7878]),
        ];
        let found = here("lemonfiber", &seen, &running());
        let order: Vec<String> = found
            .first()
            .map(|project| {
                project
                    .services
                    .iter()
                    .map(|service| service.service.clone())
                    .collect()
            })
            .unwrap_or_default();
        assert_eq!(order, vec!["radarr".to_owned(), "sonarr".to_owned()]);
    }

    #[test]
    fn a_port_we_want_that_something_else_holds_is_named_on_both_sides() {
        let seen = [container("media", "sonarr", &[8989])];
        let standing = here("lemonfiber", &seen, &running());
        let found = conflicts(&running(), &standing);
        let held = found
            .first()
            .map(|clash| (clash.port, clash.wanted_by.clone(), clash.held_by.clone()));
        assert_eq!(
            held,
            Some((8989, "sonarr".to_owned(), "media/sonarr".to_owned())),
            "{found:?}"
        );
    }

    #[test]
    fn a_port_nobody_else_holds_is_not_a_conflict() {
        let seen = [container("media", "sonarr", &[1234])];
        let standing = here("lemonfiber", &seen, &running());
        assert!(conflicts(&running(), &standing).is_empty());
    }

    #[test]
    fn a_service_of_ours_that_publishes_nothing_can_collide_with_nothing() {
        let seen = [container("media", "sonarr", &[8989])];
        let standing = here("lemonfiber", &seen, &running());
        let quiet = [ours("recyclarr", "recyclarr", "1", None)];
        assert!(conflicts(&quiet, &standing).is_empty());
    }

    #[test]
    fn conflicts_read_lowest_port_first_whoever_holds_them() {
        let seen = [
            container("media", "sonarr", &[8989]),
            container("shop", "postgres", &[7878]),
        ];
        let standing = here("lemonfiber", &seen, &running());
        let order: Vec<u16> = conflicts(&running(), &standing)
            .iter()
            .map(|clash| clash.port)
            .collect();
        assert_eq!(order, vec![7878, 8989]);
    }

    #[test]
    fn every_port_standing_here_is_what_a_second_copy_must_step_over() {
        let seen = [container("media", "sonarr", &[8989, 9999])];
        let standing = here("lemonfiber", &seen, &running());
        let held = taken(&standing);
        assert!(held.contains(&8989) && held.contains(&9999), "{held:?}");
    }

    #[test]
    fn a_service_we_do_not_run_beside_one_we_do_is_named_rather_than_passed_over() {
        let seen = [
            container("media", "sonarr", &[8989]),
            container("media", "ombi", &[3579]),
        ];
        let standing = here("lemonfiber", &seen, &running());
        let named = unsupported(&standing).first().map(|item| item.what.clone());
        assert_eq!(named, Some("media/ombi".to_owned()));
    }

    #[test]
    fn somebody_elses_unrelated_project_is_not_a_stack_we_failed_to_adopt() {
        let seen = [
            container("shop", "postgres", &[5432]),
            container("shop", "redis", &[6379]),
        ];
        let standing = here("lemonfiber", &seen, &running());
        assert!(unsupported(&standing).is_empty());
    }
}
