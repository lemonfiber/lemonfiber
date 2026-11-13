use super::{conflicts, here, taken, unsupported};
use crate::migration::tests::{container, ours, running};
use crate::migration::Ours;
use crate::model::{Location, OccupantReport};
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

fn mounted(source: &str, destination: &str) -> crate::ports::docker::Mounted {
    crate::ports::docker::Mounted {
        source: std::path::PathBuf::from(source),
        destination: std::path::PathBuf::from(destination),
    }
}

fn occupant(seen: crate::ports::docker::Container, ours: &[Ours]) -> Option<OccupantReport> {
    here("lemonfiber", &[seen], ours)
        .first()
        .and_then(|project| project.services.first())
        .cloned()
}

#[test]
fn a_known_service_says_where_its_configuration_and_its_library_are_mounted_from() {
    let mut seen = container("media", "sonarr", &[8989]);
    seen.mounts = vec![
        mounted("/srv/media", "/data"),
        mounted("/srv/apps/sonarr", "/config"),
    ];

    let found = occupant(seen, &running());

    assert_eq!(
        found.map(|one| (one.config, one.library)),
        Some((
            Location::At("/srv/apps/sonarr".to_owned()),
            Location::At("/srv/media".to_owned())
        ))
    );
}

#[test]
fn the_configuration_is_read_where_the_manifest_says_the_service_keeps_it() {
    let mut seen = container("media", "sonarr", &[8989]);
    seen.mounts = vec![
        mounted("/srv/apps/sonarr", "/config"),
        mounted("/srv/apps/sonarr-own", "/settings"),
    ];
    let mut kept_elsewhere = ours("sonarr", "linuxserver/sonarr", "4.0.1", Some(8989));
    kept_elsewhere.configuration = "/settings".to_owned();

    let found = occupant(seen, &[kept_elsewhere]);

    assert_eq!(
        found.map(|one| one.config),
        Some(Location::At("/srv/apps/sonarr-own".to_owned()))
    );
}

#[test]
fn nothing_mounted_where_a_service_keeps_something_is_said_unread_naming_where() {
    let mut seen = container("media", "sonarr", &[8989]);
    seen.mounts = vec![mounted("/srv/tv", "/tv")];

    let found = occupant(seen, &running());

    let Some(found) = found else {
        unreachable!("the service was not found");
    };
    assert!(
        matches!(&found.config, Location::Unread(why) if why.contains("/config")),
        "{found:?}"
    );
    assert!(
        matches!(&found.library, Location::Unread(why) if why.contains("/data")),
        "{found:?}"
    );
}

#[test]
fn a_service_that_takes_no_data_keeps_no_library() {
    let mut seen = container("media", "prowlarr", &[9696]);
    seen.mounts = vec![mounted("/srv/apps/prowlarr", "/config")];
    let mut indexer = ours("prowlarr", "linuxserver/prowlarr", "1.0.0", Some(9696));
    indexer.takes_data = false;

    let found = occupant(seen, &[indexer]);

    let Some(found) = found else {
        unreachable!("the service was not found");
    };
    assert_eq!(found.config, Location::At("/srv/apps/prowlarr".to_owned()));
    assert!(
        matches!(&found.library, Location::None(why) if why.contains("no library")),
        "{found:?}"
    );
}

#[test]
fn a_service_lemonfiber_does_not_know_has_both_locations_unread() {
    let mut seen = container("media", "postgres", &[5432]);
    seen.mounts = vec![mounted("/srv/db", "/config")];

    let found = occupant(seen, &running());

    let Some(found) = found else {
        unreachable!("the service was not found");
    };
    assert!(matches!(found.config, Location::Unread(_)), "{found:?}");
    assert!(matches!(found.library, Location::Unread(_)), "{found:?}");
}
