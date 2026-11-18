use super::{retried, settled, unanswered, UNANSWERED};
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

/// A pass on the paused clock: `FlareSolverr into Prowlarr` not answering until `answers`
/// has gone by, wired from then on.
fn solver_answering(
    answers: std::time::Duration,
) -> impl FnMut() -> std::future::Ready<Result<Report, Box<crate::error::Problem>>> {
    let started = tokio::time::Instant::now();
    move || {
        let state = if started.elapsed() >= answers {
            State::Wired
        } else {
            State::Skipped {
                reason: NOT_ANSWERING.to_owned(),
            }
        };
        std::future::ready(Ok(passed(vec![one("FlareSolverr into Prowlarr", state)])))
    }
}

/// A solver whose cold start outlasts two later passes is wired by a pass after it, inside
/// the window.
#[tokio::test(start_paused = true)]
async fn a_service_answering_within_the_window_is_wired_by_a_later_pass() {
    let first = passed(vec![silent("FlareSolverr into Prowlarr")]);
    let started = tokio::time::Instant::now();

    let reported = settled(
        first,
        UNANSWERED,
        solver_answering(std::time::Duration::from_secs(34)),
    )
    .await;

    assert_eq!(
        reported.map(|reported| reported.wirings),
        Ok(vec![one("FlareSolverr into Prowlarr", State::Wired)])
    );
    assert_eq!(started.elapsed(), UNANSWERED.between * 3);
}

/// A service that never answers is tried as often as the window allows and no more, and
/// is reported not answering.
#[tokio::test(start_paused = true)]
async fn a_service_never_answering_is_left_skipped_once_the_window_closes() {
    let first = passed(vec![silent("FlareSolverr into Prowlarr")]);
    let started = tokio::time::Instant::now();

    let reported = settled(
        first,
        UNANSWERED,
        solver_answering(std::time::Duration::MAX),
    )
    .await;

    assert_eq!(
        reported.map(|reported| reported.wirings),
        Ok(vec![silent("FlareSolverr into Prowlarr")])
    );
    assert_eq!(started.elapsed(), UNANSWERED.longest());
}

/// A pass with nothing left not answering asks nothing more and waits for nothing.
#[tokio::test(start_paused = true)]
async fn a_pass_with_every_service_answering_is_not_tried_again() {
    let first = passed(vec![one("SABnzbd into Radarr", State::Wired)]);
    let started = tokio::time::Instant::now();

    let reported = settled(
        first,
        UNANSWERED,
        solver_answering(std::time::Duration::MAX),
    )
    .await;

    assert_eq!(
        reported.map(|reported| reported.wirings),
        Ok(vec![one("SABnzbd into Radarr", State::Wired)])
    );
    assert_eq!(started.elapsed(), std::time::Duration::ZERO);
}
