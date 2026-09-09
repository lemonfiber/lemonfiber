//! What is already on this machine, read before anything is proposed.
//!
//! Migration opens as a survey. lemonfiber looks for a setup already here and states
//! what it found; nothing in this module writes, and nothing in it chooses. Reading is
//! kept apart from acting so that looking can never be the step that breaks a working
//! stack.
//!
//! Nothing here can reach an engine. It is given what was read and returns what that
//! amounts to, which is what lets the whole survey be exercised against arrangements
//! that would take a machine-day to stand up for real.
//!
//! The parts are separate because they answer separate questions — [`standing`] what is
//! here, [`carrying`] what taking it over would cost, [`mode`] what may be done about
//! it, [`image`] what an image is, [`version`] which of two versions is later — and
//! this module is where one survey is assembled out of all of them.

use crate::model::MigrationReport;
use crate::ports::docker::{Container, Image};

pub mod carrying;
pub mod image;
pub mod importing;
pub mod linking;
pub mod mode;
pub mod standing;
pub mod version;

/// One service lemonfiber runs, as its own manifest declares it.
///
/// One type rather than the several parallel lists this began as. Every question the
/// survey asks of lemonfiber's own stack — which services it knows, which ports it
/// would take, which images and versions it pins — is a field of the same row, and
/// splitting them into lists that had to be kept in step was how a service could be
/// known for one question and missing from the next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ours {
    /// The service, by its manifest id, which is also its Compose service name.
    pub service: String,
    /// The image reference it runs, without a tag.
    pub image: String,
    /// The tag it pins.
    pub tag: String,
    /// The host port it would publish, absent for a service with no listener.
    pub port: Option<u16>,
}

/// What is already here, given what the engine reported.
///
/// `project` is lemonfiber's own Compose project, which is excluded: the survey is
/// about what somebody else built.
///
/// The whole report is assembled here rather than partly by the caller. A survey put
/// together in two places is one a second caller finishes differently, and the field it
/// forgets reads as an empty answer rather than a missing one.
#[must_use]
pub fn surveyed(
    project: &str,
    seen: &[Container],
    images: &[Image],
    ours: &[Ours],
    mounted: &[(std::path::PathBuf, crate::ports::filesystem::StorageFacts)],
) -> MigrationReport {
    let standing = standing::here(project, seen, ours);

    let mut unsupported = standing::unsupported(&standing);
    unsupported.extend(image::outside_compose(images, ours));

    let carried = standing
        .iter()
        .flat_map(|project| carrying::carrying(images, &project.project, ours))
        .collect();

    MigrationReport {
        read: true,
        conflicts: standing::conflicts(ours, &standing),
        beside: mode::beside(ours, &standing::taken(&standing)),
        modes: mode::offered(),
        not_carried: carrying::not_carried(),
        carrying: carried,
        linking: linking::linking(mounted),
        unsupported,
        standing,
    }
}

/// The one setup here that lemonfiber could act on, where there is exactly one.
///
/// Exactly one, because adopting, standing beside and standing in place of are all
/// choices about *which* stack, and a machine holding two is one where only the operator
/// can say which they meant.
///
/// It has to hold something lemonfiber runs. A project of somebody's own — a database, a
/// cache, the app they are writing — is not a media stack any of these acts is about, and
/// acting on it because it happened to be the only thing here is the most expensive
/// misreading in this family.
#[must_use]
pub fn one_setup(survey: &MigrationReport) -> Option<String> {
    let mut candidates = survey
        .standing
        .iter()
        .filter(|project| project.services.iter().any(|service| service.adoptable));
    let first = candidates.next()?;
    if candidates.next().is_some() {
        return None;
    }
    Some(first.project.clone())
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
        modes: Vec::new(),
        beside: Vec::new(),
        linking: None,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{surveyed, unread, Ours};
    use crate::ports::docker::{Container, Health, Image, Lifecycle, Published};
    use std::net::{IpAddr, Ipv4Addr};

    /// One container of somebody's stack, publishing the given host ports.
    pub(crate) fn container(project: &str, service: &str, ports: &[u16]) -> Container {
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
            mounts: Vec::new(),
            exit: None,
        }
    }

    /// One image the engine has pulled, and the projects standing on it.
    pub(crate) fn image(tags: &[&str], projects: &[&str]) -> Image {
        Image {
            tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
            bytes: 1,
            projects: projects.iter().map(|name| (*name).to_owned()).collect(),
        }
    }

    /// One service lemonfiber runs.
    pub(crate) fn ours(service: &str, image: &str, tag: &str, port: Option<u16>) -> Ours {
        Ours {
            service: service.to_owned(),
            image: image.to_owned(),
            tag: tag.to_owned(),
            port,
        }
    }

    /// The two \*arrs most of these cases are about.
    pub(crate) fn running() -> Vec<Ours> {
        vec![
            ours("sonarr", "linuxserver/sonarr", "4.0.1", Some(8989)),
            ours("radarr", "linuxserver/radarr", "5.0.1", Some(7878)),
        ]
    }

    /// One setup holding something lemonfiber runs is the one acted on.
    #[test]
    fn the_one_setup_holding_our_services_is_the_one_acted_on() {
        let seen = [container("media", "sonarr", &[8989])];
        let found = surveyed("lemonfiber", &seen, &[], &running(), &[]);
        assert_eq!(super::one_setup(&found), Some("media".to_owned()));
    }

    /// Somebody's own work is not a media stack any of these acts is about.
    #[test]
    fn a_project_holding_nothing_of_ours_is_not_acted_on() {
        let seen = [container("shop", "postgres", &[5432])];
        let found = surveyed("lemonfiber", &seen, &[], &running(), &[]);
        assert_eq!(super::one_setup(&found), None);
    }

    /// Two of them is a question only the operator can answer.
    #[test]
    fn two_setups_holding_our_services_is_not_a_choice_lemonfiber_makes() {
        let seen = [
            container("media", "sonarr", &[8989]),
            container("archive", "radarr", &[7878]),
        ];
        let found = surveyed("lemonfiber", &seen, &[], &running(), &[]);
        assert_eq!(super::one_setup(&found), None);
    }

    /// An unrelated project beside ours still leaves exactly one candidate.
    #[test]
    fn an_unrelated_project_beside_one_of_ours_does_not_make_it_ambiguous() {
        let seen = [
            container("media", "sonarr", &[8989]),
            container("shop", "redis", &[6379]),
        ];
        let found = surveyed("lemonfiber", &seen, &[], &running(), &[]);
        assert_eq!(super::one_setup(&found), Some("media".to_owned()));
    }

    #[test]
    fn a_survey_that_looked_and_found_nothing_says_it_looked() {
        let found = surveyed("lemonfiber", &[], &[], &running(), &[]);
        assert!(found.read);
        assert!(found.standing.is_empty());
    }

    #[test]
    fn an_engine_that_would_not_say_is_unread_rather_than_empty() {
        let refused = unread();
        assert!(!refused.read);
        assert!(refused.standing.is_empty());
    }

    #[test]
    fn a_survey_states_what_no_migration_carries_across_whatever_it_found() {
        let found = surveyed("lemonfiber", &[], &[], &running(), &[]);
        assert!(!found.not_carried.is_empty(), "{found:?}");
        assert!(!found.modes.is_empty(), "{found:?}");
    }

    /// The whole point of assembling in one place: every part of the answer is filled
    /// by the one call, so a caller cannot half-finish it.
    #[test]
    fn one_call_fills_every_part_of_the_answer() {
        let seen = [
            container("media", "sonarr", &[8989]),
            container("media", "ombi", &[3579]),
        ];
        let images = [image(&["linuxserver/sonarr:4.0.9"], &["media"])];
        let found = surveyed("lemonfiber", &seen, &images, &running(), &[]);

        assert!(!found.standing.is_empty(), "what is here");
        assert!(!found.conflicts.is_empty(), "what collides");
        assert!(!found.unsupported.is_empty(), "what cannot be adopted");
        assert!(!found.carrying.is_empty(), "what taking it over costs");
        assert!(!found.not_carried.is_empty(), "what never carries");
        assert!(!found.modes.is_empty(), "what may be done");
        assert!(!found.beside.is_empty(), "where a second copy would listen");
    }

    #[test]
    fn a_second_copy_steps_over_the_ports_the_existing_stack_holds() {
        let seen = [container("media", "sonarr", &[8989, 8990])];
        let found = surveyed("lemonfiber", &seen, &[], &running(), &[]);
        let sonarr = found
            .beside
            .iter()
            .find(|moved| moved.service == "sonarr")
            .map(|moved| moved.to);
        assert_eq!(sonarr, Some(8991), "{:?}", found.beside);
    }
}
