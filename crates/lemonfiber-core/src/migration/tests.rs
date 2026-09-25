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
