use super::{blocked, layered};
use crate::model::{MigrationReport, MovedReport};

fn moved(service: &str, from: u16, to: u16) -> MovedReport {
    MovedReport {
        service: service.to_owned(),
        from,
        to,
    }
}

/// The host port moves and the container port does not: what a service listens on
/// inside itself is the image's business, and rewriting it would break the service
/// rather than move it.
#[test]
fn only_the_host_side_of_a_port_is_moved() {
    let written = layered(&[moved("sonarr", 8989, 8990)]);
    assert!(written.contains("\"8990:8989\""), "{written}");
}

/// Every service that moved, under the one key Compose merges by.
#[test]
fn each_service_that_moved_is_named_under_services() {
    let written = layered(&[moved("sonarr", 8989, 8990), moved("radarr", 7878, 7879)]);
    assert!(written.starts_with("services:\n"), "{written}");
    assert!(written.contains("  sonarr:\n"), "{written}");
    assert!(written.contains("  radarr:\n"), "{written}");
}

/// Nothing but ports, so the overlay cannot go stale against the stack's own file.
#[test]
fn nothing_but_ports_is_restated() {
    let written = layered(&[moved("sonarr", 8989, 8990)]);
    assert!(!written.contains("image"), "{written}");
    assert!(!written.contains("volumes"), "{written}");
}

/// A machine it could not read is one whose free ports it would be guessing at.
#[test]
fn a_survey_that_could_not_look_stops_it() {
    let said = blocked(&MigrationReport::default())
        .and_then(|read| read.refusal)
        .unwrap_or_default();
    assert!(said.contains("could not be read"), "{said}");
}

/// Nowhere left to listen is a refusal rather than an empty file.
#[test]
fn nowhere_left_to_listen_stops_it() {
    let looked = MigrationReport {
        read: true,
        ..MigrationReport::default()
    };
    let said = blocked(&looked)
        .and_then(|read| read.refusal)
        .unwrap_or_default();
    assert!(said.contains("anywhere else to listen"), "{said}");
}

/// Somewhere to go is what lets it proceed.
#[test]
fn somewhere_to_listen_stops_nothing() {
    let looked = MigrationReport {
        read: true,
        beside: vec![moved("sonarr", 8989, 8990)],
        ..MigrationReport::default()
    };
    assert!(blocked(&looked).is_none());
}
