//! What is already on this machine, read before anything is proposed.
//!
//! Migration opens as a survey. lemonfiber looks for an existing setup and states what it
//! found; nothing here writes, and nothing here chooses. Reading is kept apart from
//! acting so that looking can never be the step that breaks a working stack.
//!
//! Nothing in this module can reach an engine. It is given what was read and returns
//! what that amounts to, which is what lets the whole survey be exercised against
//! arrangements that would take a machine-day to stand up for real.

use std::collections::BTreeMap;

use crate::model::{
    CarryingReport, ConflictReport, MigrationReport, OccupantReport, StandingReport,
    UnsupportedReport,
};
use crate::ports::docker::{Container, Image, Lifecycle};

pub mod version;

use version::Standing;

/// A port lemonfiber's own stack would publish, and the service that would publish it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wanted {
    /// The lemonfiber service, by its manifest id.
    pub service: String,
    /// The host port it would answer on.
    pub port: u16,
}

/// What is already here, given what the engine reported.
///
/// `ours` is lemonfiber's own Compose project, which is excluded: the survey is about
/// what somebody else built. `known` is every service lemonfiber can stand up, which is
/// what decides whether an existing container could be taken over as it stands.
#[must_use]
pub fn surveyed(
    ours: &str,
    seen: &[Container],
    wanted: &[Wanted],
    known: &[String],
) -> MigrationReport {
    let mut projects: BTreeMap<&str, Vec<OccupantReport>> = BTreeMap::new();
    for container in seen.iter().filter(|found| found.project != ours) {
        projects
            .entry(&container.project)
            .or_default()
            .push(occupant(container, known));
    }

    let standing: Vec<StandingReport> = projects
        .into_iter()
        .map(|(project, mut services)| {
            services.sort_by(|one, two| one.service.cmp(&two.service));
            StandingReport {
                project: project.to_owned(),
                services,
            }
        })
        .collect();

    MigrationReport {
        read: true,
        conflicts: conflicts(wanted, &standing),
        unsupported: unsupported(&standing),
        not_carried: not_carried(),
        carrying: Vec::new(),
        standing,
    }
}

/// Every service of a recognisable setup that lemonfiber could not take over.
///
/// Only of a project already holding at least one service lemonfiber runs. A project
/// with none of them is somebody's unrelated work rather than a stack being migrated,
/// and naming its every service unsupported would say lemonfiber had weighed adopting a
/// database it was never asked about. Such a project is still reported as standing here,
/// because the ports it holds are just as taken either way.
fn unsupported(standing: &[StandingReport]) -> Vec<UnsupportedReport> {
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

/// One existing container, as it appears in the survey.
fn occupant(container: &Container, known: &[String]) -> OccupantReport {
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
        adoptable: known.iter().any(|service| service == &container.service),
    }
}

/// Every port lemonfiber wants that an existing service already answers on.
///
/// Reported before a plan rather than discovered while applying one, because a port
/// already taken is the ordinary way a second stack fails to start, and finding it then
/// leaves the operator with two half-running setups.
fn conflicts(wanted: &[Wanted], standing: &[StandingReport]) -> Vec<ConflictReport> {
    let mut found: Vec<ConflictReport> = wanted
        .iter()
        .flat_map(|want| {
            standing.iter().flat_map(move |project| {
                project
                    .services
                    .iter()
                    .filter(move |occupant| occupant.ports.contains(&want.port))
                    .map(move |occupant| ConflictReport {
                        port: want.port,
                        wanted_by: want.service.clone(),
                        held_by: format!("{}/{}", project.project, occupant.service),
                    })
            })
        })
        .collect();
    found.sort_by(|one, two| one.port.cmp(&two.port).then(one.held_by.cmp(&two.held_by)));
    found
}

/// One service lemonfiber runs, and the image and tag it pins for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pinned {
    /// The service, by its manifest id.
    pub service: String,
    /// The image reference, without a tag.
    pub image: String,
    /// The tag lemonfiber pins.
    pub tag: String,
}

/// What no migration carries across, in any mode.
///
/// Stated rather than discovered, because these are the things an operator finds
/// missing weeks later and has no way to connect back to the day they migrated. A
/// migration that implied it took everything would be the more comfortable claim and
/// the one that costs them that afternoon.
const NEVER_CARRIED: [(&str, &str); 4] = [
    (
        "custom formats you wrote yourself",
        "they are scoring rules particular to your library, and lemonfiber sets its own from the \
         quality preset you choose",
    ),
    (
        "per-indexer tuning",
        "seed ratios, limits and priorities are set against indexers lemonfiber does not know you \
         hold accounts with",
    ),
    (
        "scripts hooked into an *arr's events",
        "a connect script runs a program on your machine, and carrying one across would run \
         somebody's program somewhere it was never pointed at",
    ),
    (
        "watch histories and play state",
        "they live in the media server's own database rather than in the wiring lemonfiber sets \
         up, and nothing here reads them",
    ),
];

/// What no migration carries across, whatever mode it runs in.
#[must_use]
pub fn not_carried() -> Vec<UnsupportedReport> {
    NEVER_CARRIED
        .iter()
        .map(|(what, because)| UnsupportedReport {
            what: (*what).to_owned(),
            because: (*because).to_owned(),
        })
        .collect()
}

/// What adopting each service lemonfiber recognises would come to.
///
/// The tag standing on the existing project decides it: an \*arr migrates its database
/// forward on first start, and the binary that did so is the oldest that can open it
/// afterwards. So a later version already here is a door lemonfiber cannot walk back
/// through, and it is refused rather than attempted.
#[must_use]
pub fn carrying(images: &[Image], project: &str, ours: &[Pinned]) -> Vec<CarryingReport> {
    let mut found: Vec<CarryingReport> = ours
        .iter()
        .filter_map(|pinned| {
            let existing = standing_on(images, project, &pinned.image)?;
            Some(verdict(pinned, &existing))
        })
        .collect();
    found.sort_by(|one, two| one.service.cmp(&two.service));
    found
}

/// The tag of the image behind one of lemonfiber's services on a given project.
fn standing_on(images: &[Image], project: &str, image: &str) -> Option<String> {
    images
        .iter()
        .filter(|pulled| pulled.projects.iter().any(|held| held == project))
        .find_map(|pulled| {
            pulled
                .tags
                .iter()
                .find(|tag| repository(tag) == image)
                .map(|tag| version_of(tag).to_owned())
        })
}

/// The version part of a tag, which is what is left once the repository is dropped.
fn version_of(tag: &str) -> &str {
    let repository = repository(tag);
    tag.get(repository.len()..)
        .map_or(tag, |rest| rest.strip_prefix(':').unwrap_or(tag))
}

/// What adopting one service would come to, given the version already here.
fn verdict(pinned: &Pinned, existing: &str) -> CarryingReport {
    let (verdict, because, backup_first, refused) = match version::against(existing, &pinned.tag) {
        Standing::Same => (
            "same",
            "the version already here is the one lemonfiber runs, so its database is opened \
             exactly as it stands"
                .to_owned(),
            false,
            false,
        ),
        Standing::Earlier => (
            "upgrade",
            format!(
                "lemonfiber runs {}, which upgrades this database on first start and is a step \
                 nothing walks back — so it is backed up before anything opens it",
                pinned.tag
            ),
            true,
            false,
        ),
        Standing::Later => (
            "downgrade",
            format!(
                "this database has been through {existing}, and {} cannot open it afterwards — \
                 lemonfiber will not try, because the attempt is what damages it",
                pinned.tag
            ),
            false,
            true,
        ),
        Standing::Untellable => (
            "cannot tell",
            format!(
                "neither {existing} nor {} reads as a version, so which came first cannot be \
                 told from the tags — it is backed up first rather than assumed safe",
                pinned.tag
            ),
            true,
            false,
        ),
    };

    CarryingReport {
        service: pinned.service.clone(),
        existing: existing.to_owned(),
        ours: pinned.tag.clone(),
        verdict: verdict.to_owned(),
        because,
        backup_first,
        refused,
    }
}

/// The repository part of an image tag, with any version dropped.
///
/// A registry may itself carry a port, so only a final segment holding no path
/// separator is a tag rather than part of the address.
fn repository(tag: &str) -> &str {
    match tag.rsplit_once(':') {
        Some((repository, version)) if !version.contains('/') => repository,
        _ => tag,
    }
}

/// Every service lemonfiber knows that is running outside Compose.
///
/// A container started by hand carries no project for the engine to report, so it
/// cannot be listed the way a project can, and it is named from the image beneath it
/// instead. Narrowed to images lemonfiber runs: a machine has databases and build tools
/// standing on it that have nothing to do with a media stack, and naming those under a
/// migration survey would bury the one line that matters — a Jellyfin nobody can adopt
/// because there is no project description to adopt it from.
#[must_use]
pub fn outside_compose(images: &[Image], known: &[String]) -> Vec<UnsupportedReport> {
    let mut named: Vec<UnsupportedReport> = images
        .iter()
        .filter(|image| image.projects.iter().any(String::is_empty))
        .filter_map(|image| {
            image
                .tags
                .iter()
                .find(|tag| known.iter().any(|ours| ours == repository(tag)))
                .cloned()
        })
        .map(|what| UnsupportedReport {
            what,
            because: "it was started outside Compose, so there is no project description \
                      to take it over from"
                .to_owned(),
        })
        .collect();
    named.sort_by(|one, two| one.what.cmp(&two.what));
    named
}

/// What the survey answers when the engine would not say.
///
/// Empty, and saying so. A survey that reported no existing setup because it could not
/// look would invite a replacement over the top of a library that is still there.
#[must_use]
pub const fn unread() -> MigrationReport {
    MigrationReport {
        read: false,
        standing: Vec::new(),
        conflicts: Vec::new(),
        unsupported: Vec::new(),
        carrying: Vec::new(),
        not_carried: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use super::{carrying, not_carried, outside_compose, surveyed, unread, Pinned, Wanted};
    use crate::ports::docker::{Container, Health, Image, Lifecycle, Published};

    fn image(tags: &[&str], projects: &[&str]) -> Image {
        Image {
            tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
            bytes: 1,
            projects: projects.iter().map(|name| (*name).to_owned()).collect(),
        }
    }

    fn container(project: &str, service: &str, ports: &[u16]) -> Container {
        Container {
            id: format!("{project}-{service}"),
            project: project.to_owned(),
            service: service.to_owned(),
            lifecycle: Lifecycle::Running,
            health: Health::None,
            published: ports
                .iter()
                .map(|port| Published {
                    address: IpAddr::V4(Ipv4Addr::LOCALHOST),
                    port: *port,
                })
                .collect(),
            exit: None,
        }
    }

    fn known() -> Vec<String> {
        vec!["sonarr".to_owned(), "radarr".to_owned()]
    }

    fn pulled() -> Vec<String> {
        vec!["plex".to_owned()]
    }

    #[test]
    fn our_own_project_is_not_somebody_elses_setup() {
        let seen = [container("lemonfiber", "sonarr", &[8989])];
        let found = surveyed("lemonfiber", &seen, &[], &known());
        assert!(found.standing.is_empty(), "{:?}", found.standing);
    }

    #[test]
    fn an_existing_project_is_reported_with_the_ports_it_answers_on() {
        let seen = [container("media", "sonarr", &[8989])];
        let found = surveyed("lemonfiber", &seen, &[], &known());
        let one = found
            .standing
            .first()
            .map(|project| project.project.clone());
        assert_eq!(one, Some("media".to_owned()), "{:?}", found.standing);
        let ports = found
            .standing
            .first()
            .and_then(|project| project.services.first())
            .map(|service| service.ports.clone());
        assert_eq!(ports, Some(vec![8989]), "{:?}", found.standing);
    }

    #[test]
    fn a_port_we_want_that_something_else_holds_is_named_on_both_sides() {
        let seen = [container("media", "sonarr", &[8989])];
        let want = [Wanted {
            service: "sonarr".to_owned(),
            port: 8989,
        }];
        let found = surveyed("lemonfiber", &seen, &want, &known());
        let held = found
            .conflicts
            .first()
            .map(|clash| (clash.port, clash.wanted_by.clone(), clash.held_by.clone()));
        assert_eq!(
            held,
            Some((8989, "sonarr".to_owned(), "media/sonarr".to_owned())),
            "{:?}",
            found.conflicts
        );
    }

    #[test]
    fn conflicts_read_lowest_port_first_whoever_holds_them() {
        let seen = [
            container("media", "sonarr", &[8989]),
            container("shop", "postgres", &[7878]),
        ];
        let want = [
            Wanted {
                service: "sonarr".to_owned(),
                port: 8989,
            },
            Wanted {
                service: "radarr".to_owned(),
                port: 7878,
            },
        ];
        let found = surveyed("lemonfiber", &seen, &want, &known());
        let order: Vec<u16> = found.conflicts.iter().map(|clash| clash.port).collect();
        assert_eq!(order, vec![7878, 8989], "{:?}", found.conflicts);
    }

    #[test]
    fn what_was_started_outside_compose_reads_in_a_settled_order() {
        let images = [image(&["sonarr:1"], &[""]), image(&["plex:latest"], &[""])];
        let known = ["sonarr".to_owned(), "plex".to_owned()];
        let named: Vec<String> = outside_compose(&images, &known)
            .into_iter()
            .map(|item| item.what)
            .collect();
        assert_eq!(named, vec!["plex:latest".to_owned(), "sonarr:1".to_owned()]);
    }

    #[test]
    fn a_port_nobody_else_holds_is_not_a_conflict() {
        let seen = [container("media", "sonarr", &[8989])];
        let want = [Wanted {
            service: "radarr".to_owned(),
            port: 7878,
        }];
        let found = surveyed("lemonfiber", &seen, &want, &known());
        assert!(found.conflicts.is_empty(), "{:?}", found.conflicts);
    }

    #[test]
    fn a_service_we_do_not_run_beside_one_we_do_is_named_rather_than_passed_over() {
        let seen = [
            container("media", "sonarr", &[8989]),
            container("media", "ombi", &[3579]),
        ];
        let found = surveyed("lemonfiber", &seen, &[], &known());
        let named = found.unsupported.first().map(|item| item.what.clone());
        assert_eq!(
            named,
            Some("media/ombi".to_owned()),
            "{:?}",
            found.unsupported
        );
    }

    #[test]
    fn somebody_elses_unrelated_project_is_not_a_stack_we_failed_to_adopt() {
        let seen = [
            container("shop", "postgres", &[5432]),
            container("shop", "redis", &[6379]),
        ];
        let found = surveyed("lemonfiber", &seen, &[], &known());
        assert!(found.unsupported.is_empty(), "{:?}", found.unsupported);
    }

    #[test]
    fn an_unrelated_project_still_holds_the_ports_it_holds() {
        let seen = [container("shop", "postgres", &[8989])];
        let want = [Wanted {
            service: "sonarr".to_owned(),
            port: 8989,
        }];
        let found = surveyed("lemonfiber", &seen, &want, &known());
        let held = found.conflicts.first().map(|clash| clash.held_by.clone());
        assert_eq!(
            held,
            Some("shop/postgres".to_owned()),
            "{:?}",
            found.conflicts
        );
    }

    #[test]
    fn a_container_started_by_hand_is_named_from_the_image_beneath_it() {
        let images = [image(&["plex:latest"], &[""])];
        let named = outside_compose(&images, &pulled());
        let what = named.first().map(|item| item.what.clone());
        assert_eq!(what, Some("plex:latest".to_owned()), "{named:?}");
    }

    #[test]
    fn an_image_only_projects_stand_on_is_not_named_as_unsupported() {
        let images = [image(&["plex:latest"], &["media"])];
        let named = outside_compose(&images, &pulled());
        assert!(named.is_empty(), "{named:?}");
    }

    #[test]
    fn an_image_named_without_a_version_is_still_recognised() {
        let images = [image(&["plex"], &[""])];
        let named = outside_compose(&images, &pulled());
        let what = named.first().map(|item| item.what.clone());
        assert_eq!(what, Some("plex".to_owned()), "{named:?}");
    }

    #[test]
    fn a_registry_carrying_its_own_port_is_not_read_as_a_version() {
        let images = [image(&["example.test:5000/plex:1.2"], &[""])];
        let named = outside_compose(&images, &["example.test:5000/plex".to_owned()]);
        let what = named.first().map(|item| item.what.clone());
        assert_eq!(
            what,
            Some("example.test:5000/plex:1.2".to_owned()),
            "{named:?}"
        );
    }

    #[test]
    fn an_image_we_do_not_run_is_not_a_migration_finding() {
        let images = [image(&["a-database:17"], &[""])];
        let named = outside_compose(&images, &pulled());
        assert!(named.is_empty(), "{named:?}");
    }

    #[test]
    fn a_stopped_container_is_still_found_and_says_it_is_not_running() {
        let mut stopped = container("media", "sonarr", &[8989]);
        stopped.lifecycle = Lifecycle::Exited;
        let found = surveyed("lemonfiber", &[stopped], &[], &known());
        let running = found
            .standing
            .first()
            .and_then(|project| project.services.first())
            .map(|service| service.running);
        assert_eq!(running, Some(false), "{:?}", found.standing);
    }

    #[test]
    fn services_of_one_project_read_in_a_settled_order() {
        let seen = [
            container("media", "sonarr", &[8989]),
            container("media", "radarr", &[7878]),
        ];
        let found = surveyed("lemonfiber", &seen, &[], &known());
        let order: Vec<String> = found
            .standing
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
    fn a_repeated_port_is_reported_once() {
        let seen = [container("media", "sonarr", &[8989, 8989])];
        let found = surveyed("lemonfiber", &seen, &[], &known());
        let ports = found
            .standing
            .first()
            .and_then(|project| project.services.first())
            .map(|service| service.ports.clone());
        assert_eq!(ports, Some(vec![8989]), "{:?}", found.standing);
    }

    fn pinned(service: &str, image: &str, tag: &str) -> Pinned {
        Pinned {
            service: service.to_owned(),
            image: image.to_owned(),
            tag: tag.to_owned(),
        }
    }

    /// What adopting one service came to, as (verdict, backup first, refused).
    fn taking(existing: &str, ours: &str) -> Option<(String, bool, bool)> {
        let images = [image(
            &[&format!("linuxserver/sonarr:{existing}")],
            &["media"],
        )];
        let ours = [pinned("sonarr", "linuxserver/sonarr", ours)];
        carrying(&images, "media", &ours)
            .first()
            .map(|read| (read.verdict.clone(), read.backup_first, read.refused))
    }

    #[test]
    fn the_version_already_here_being_ours_is_taken_over_as_it_stands() {
        assert_eq!(
            taking("4.0.1", "4.0.1"),
            Some(("same".to_owned(), false, false))
        );
    }

    #[test]
    fn an_older_database_is_upgraded_and_backed_up_before_anything_opens_it() {
        assert_eq!(
            taking("4.0.1", "4.0.2"),
            Some(("upgrade".to_owned(), true, false))
        );
    }

    #[test]
    fn a_database_newer_than_ours_is_refused_rather_than_attempted() {
        assert_eq!(
            taking("4.0.2", "4.0.1"),
            Some(("downgrade".to_owned(), false, true))
        );
    }

    #[test]
    fn versions_that_cannot_be_ordered_are_backed_up_rather_than_assumed_safe() {
        assert_eq!(
            taking("latest", "4.0.1"),
            Some(("cannot tell".to_owned(), true, false))
        );
    }

    #[test]
    fn a_refusal_names_both_versions_so_it_can_be_argued_with() {
        let images = [image(&["linuxserver/sonarr:4.0.9"], &["media"])];
        let ours = [pinned("sonarr", "linuxserver/sonarr", "4.0.1")];
        let said = carrying(&images, "media", &ours)
            .first()
            .map(|read| read.because.clone())
            .unwrap_or_default();
        assert!(said.contains("4.0.9"), "{said}");
        assert!(said.contains("4.0.1"), "{said}");
    }

    #[test]
    fn a_service_we_run_that_is_not_on_this_project_is_not_reported_as_carried() {
        let images = [image(&["linuxserver/sonarr:4.0.1"], &["other"])];
        let ours = [pinned("sonarr", "linuxserver/sonarr", "4.0.1")];
        assert!(carrying(&images, "media", &ours).is_empty());
    }

    #[test]
    fn what_would_be_carried_reads_in_a_settled_order() {
        let images = [
            image(&["linuxserver/sonarr:4.0.1"], &["media"]),
            image(&["linuxserver/radarr:5.0.1"], &["media"]),
        ];
        let ours = [
            pinned("sonarr", "linuxserver/sonarr", "4.0.1"),
            pinned("radarr", "linuxserver/radarr", "5.0.1"),
        ];
        let order: Vec<String> = carrying(&images, "media", &ours)
            .into_iter()
            .map(|read| read.service)
            .collect();
        assert_eq!(order, vec!["radarr".to_owned(), "sonarr".to_owned()]);
    }

    #[test]
    fn what_never_carries_across_is_named_rather_than_left_to_be_discovered() {
        let named = not_carried();
        assert!(named.len() >= 4, "{named:?}");
        let all: String = named.iter().map(|item| item.what.clone()).collect();
        assert!(all.contains("custom formats"), "{all}");
    }

    #[test]
    fn a_survey_states_what_no_migration_carries_across() {
        let found = surveyed("lemonfiber", &[], &[], &known());
        assert!(!found.not_carried.is_empty(), "{found:?}");
    }

    #[test]
    fn an_engine_that_would_not_say_is_unread_rather_than_empty() {
        let refused = unread();
        assert!(!refused.read);
        assert!(refused.standing.is_empty());
    }

    #[test]
    fn a_survey_that_looked_and_found_nothing_says_it_looked() {
        let found = surveyed("lemonfiber", &[], &[], &known());
        assert!(found.read);
        assert!(found.standing.is_empty());
    }
}
