use super::{retried, unanswered};
use crate::seed::{Assessment, Report, State, Wiring, NOT_ANSWERING};

/// A connection named `connection` that came to `state`.
fn one(connection: &str, state: State) -> Wiring {
    Wiring::settled(connection.to_owned(), state)
}

/// A connection skipped because its service was not answering.
fn silent(connection: &str) -> Wiring {
    one(
        connection,
        State::Skipped {
            reason: NOT_ANSWERING.to_owned(),
        },
    )
}

/// A pass that came to `wirings`.
fn passed(wirings: Vec<Wiring>) -> Report {
    Report {
        wirings,
        assessment: Assessment::Assessed,
        rehearsed: false,
    }
}

/// Only a connection skipped for a service not answering is tried again; one skipped for
/// any other reason is left as it was.
#[test]
fn only_a_connection_its_service_did_not_answer_is_tried_again() {
    let elsewhere = one(
        "Sonarr into Bazarr",
        State::Skipped {
            reason: "not in the active form".to_owned(),
        },
    );
    assert!(unanswered(&silent("FlareSolverr into Prowlarr")));
    assert!(!unanswered(&elsewhere));
    assert!(!unanswered(&one("SABnzbd into Radarr", State::Wired)));
}

/// A connection the first pass found not answering is reported as the second found it,
/// still skipped where the second did not reach it either, and every other connection is
/// reported as the first pass found it.
#[test]
fn a_connection_not_answering_is_reported_as_the_second_try_found_it() {
    let first = passed(vec![
        one("SABnzbd into Radarr", State::Wired),
        silent("FlareSolverr into Prowlarr"),
        silent("Bazarr into Radarr"),
    ]);
    let later = passed(vec![
        one("SABnzbd into Radarr", State::AlreadyWired),
        one("FlareSolverr into Prowlarr", State::Wired),
    ]);

    let reported = retried(first, &later);

    assert_eq!(
        reported.wirings,
        vec![
            one("SABnzbd into Radarr", State::Wired),
            one("FlareSolverr into Prowlarr", State::Wired),
            silent("Bazarr into Radarr"),
        ]
    );
    assert_eq!(reported.assessment, Assessment::Assessed);
}
