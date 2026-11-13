//! Registering each curator as an application in the indexer aggregator.
//!
//! The same shape again, matched by the base URL the aggregator reaches the service on.

use common::service::*;

use crate::common;
use lemonfiber_core::journal::Journal;
use lemonfiber_core::ports::service::{
    AppSync, Application, ApplicationKind, RegisteredApplication,
};
use lemonfiber_core::seed::{wire_applications, State};
use lemonfiber_fixtures::applications::Applications;

fn app(base_url: &str) -> Application {
    Application {
        name: "Sonarr".to_owned(),
        kind: ApplicationKind::Tv,
        indexer_url: "http://prowlarr:9696".to_owned(),
        base_url: base_url.to_owned(),
        api_key: "arr-key".to_owned(),
    }
}

/// Run the app-sync driver for the wanted applications, returning their states
/// and the number of changes journalled.
async fn seed_applications(
    aggregator: Applications,
    wanted: &[Application],
) -> (Vec<State>, usize) {
    let mut journal = Journal::new();
    let wirings =
        wire_applications(&aggregator, "prowlarr", wanted, &mut journal, "t", false).await;
    let states = wirings.into_iter().map(|wiring| wiring.state).collect();
    (states, journal.changes().len())
}

#[tokio::test]
async fn an_absent_application_is_registered_read_back_and_recorded() {
    let (states, recorded) = seed_applications(
        Applications::with(Mode::Normal, Vec::new()),
        &[app("http://sonarr:8989")],
    )
    .await;
    assert_eq!(states, vec![State::Wired]);
    assert_eq!(recorded, 1, "the write is journalled so it can be undone");
}

#[tokio::test]
async fn an_application_at_the_same_base_url_is_left_untouched_despite_a_different_name() {
    // Identity is the address the aggregator reaches, not the label: the operator
    // renamed the application, but it points at the same curator, so it is left
    // alone rather than registered a second time.
    let existing = vec![RegisteredApplication {
        id: "1".to_owned(),
        base_url: "http://sonarr:8989".to_owned(),
    }];
    let mut renamed = app("http://sonarr:8989");
    renamed.name = "Sonarr — my own name".to_owned();
    let (states, recorded) =
        seed_applications(Applications::with(Mode::Normal, existing), &[renamed]).await;
    assert_eq!(states, vec![State::AlreadyWired]);
    assert_eq!(
        recorded, 0,
        "an application already at the address is not duplicated"
    );
}

#[tokio::test]
async fn a_present_application_is_matched_despite_a_trailing_slash() {
    // Idempotent across the same normalization: the aggregator holds the canonical
    // address, and a wanted one that differs only by a trailing slash is left
    // alone, not re-registered.
    let existing = vec![RegisteredApplication {
        id: "1".to_owned(),
        base_url: "http://sonarr:8989".to_owned(),
    }];
    let (states, recorded) = seed_applications(
        Applications::with(Mode::Normal, existing),
        &[app("http://sonarr:8989/")],
    )
    .await;
    assert_eq!(states, vec![State::AlreadyWired]);
    assert_eq!(recorded, 0);
}

#[tokio::test]
async fn an_unavailable_aggregator_skips_every_application() {
    let (states, recorded) = seed_applications(
        Applications::with(Mode::Down, Vec::new()),
        &[app("http://sonarr:8989"), app("http://radarr:7878")],
    )
    .await;
    // Counted before it is judged: `all` over an empty list is true, so a run that
    // produced no state at all would pass a test named for skipping every application
    // while proving nothing was skipped.
    assert_eq!(states.len(), 2, "{states:?}");
    assert!(
        states
            .iter()
            .all(|state| matches!(state, State::Skipped { .. })),
        "{states:?}"
    );
    assert_eq!(recorded, 0);
}

#[tokio::test]
async fn an_aggregator_that_refuses_the_application_listing_fails() {
    let (states, _) = seed_applications(
        Applications::with(Mode::RefusesList, Vec::new()),
        &[app("http://sonarr:8989")],
    )
    .await;
    assert!(
        matches!(states.as_slice(), [State::Failed { .. }]),
        "{states:?}"
    );
}

#[tokio::test]
async fn a_rejected_application_registration_fails_with_the_aggregators_own_words() {
    let (states, recorded) = seed_applications(
        Applications::with(Mode::RejectsRegister, Vec::new()),
        &[app("http://sonarr:8989")],
    )
    .await;
    let detail = match states.as_slice() {
        [State::Failed { detail }] => Some(detail.clone()),
        _ => None,
    };
    assert!(
        detail.is_some_and(|words| words.contains("unknown implementation")),
        "the service's own words survive: {states:?}"
    );
    assert_eq!(recorded, 0, "a rejected write is not journalled");
}

#[tokio::test]
async fn an_application_write_that_does_not_appear_when_read_back_is_a_failure() {
    // Accepted but not reported back, so it did not land — not done, not recorded.
    let (states, recorded) = seed_applications(
        Applications::with(Mode::Swallows, Vec::new()),
        &[app("http://sonarr:8989")],
    )
    .await;
    assert!(
        matches!(states.as_slice(), [State::Failed { .. }]),
        "{states:?}"
    );
    assert_eq!(recorded, 0);
}

#[tokio::test]
async fn an_aggregator_that_stops_answering_after_the_write_is_skipped() {
    // The write went out but could not be confirmed, so it is left for a later
    // run to reconcile rather than declared wired.
    let (states, recorded) = seed_applications(
        Applications::with(Mode::DropsAfterRegister, Vec::new()),
        &[app("http://sonarr:8989")],
    )
    .await;
    assert!(
        matches!(states.as_slice(), [State::Skipped { .. }]),
        "{states:?}"
    );
    assert_eq!(recorded, 0, "an unconfirmed write is not recorded as done");
}

/// Run the same driver as a rehearsal: the same pass over the same aggregator, with the
/// registering left out. The journal is handed back for the half that matters most —
/// a rehearsal that recorded a change would leave the next `undo` offering to reverse
/// something nobody made.
async fn would_register(aggregator: &Applications, wanted: &[Application]) -> (Vec<State>, usize) {
    let mut journal = Journal::new();
    let wirings = wire_applications(aggregator, "prowlarr", wanted, &mut journal, "t", true).await;
    (
        wirings.into_iter().map(|wiring| wiring.state).collect(),
        journal.changes().len(),
    )
}

/// A rehearsal names the address it would register the curator at, and registers none.
///
/// The address is what identity is decided by here, so it is also the whole of what an
/// operator can check: a report saying only that an application would be added leaves
/// them unable to tell a correct pass from one about to point the aggregator at a container
/// that is not there. Read back off the aggregator afterwards rather than believed from the
/// state, because reporting `WouldWire` while still writing would look right in the
/// first assertion and be the defect this flag exists to prevent.
#[tokio::test]
async fn a_rehearsed_pass_names_the_address_it_would_register_and_registers_none() {
    let aggregator = Applications::with(Mode::Normal, Vec::new());

    let (states, recorded) = would_register(&aggregator, &[app("http://sonarr:8989")]).await;

    assert_eq!(
        states,
        vec![State::WouldWire {
            yours: None,
            ours: Some("http://sonarr:8989".to_owned()),
        }]
    );
    assert_eq!(recorded, 0, "a rehearsal journalled a change nobody made");
    let held = aggregator.applications().await.unwrap_or_default();
    assert!(held.is_empty(), "the application was registered: {held:?}");
}

/// An application whose stored key the curator no longer answers to is given the
/// current one, in place, and tested again.
#[tokio::test]
async fn an_application_on_a_replaced_key_is_given_the_current_one() {
    let existing = vec![RegisteredApplication {
        id: "1".to_owned(),
        base_url: "http://sonarr:8989".to_owned(),
    }];
    let aggregator = Applications::stale(existing.clone(), false);
    let mut journal = Journal::new();

    let wirings = wire_applications(
        &aggregator,
        "prowlarr",
        &[app("http://sonarr:8989")],
        &mut journal,
        "t",
        false,
    )
    .await;

    assert_eq!(
        wirings.into_iter().map(|one| one.state).collect::<Vec<_>>(),
        vec![State::Wired]
    );
    assert_eq!(aggregator.rekeyed(), vec!["arr-key".to_owned()]);

    let refusing = Applications::stale(existing.clone(), false).refusing_rekey();
    let (states, _) = seed_applications(refusing, &[app("http://sonarr:8989")]).await;
    assert!(
        matches!(states.as_slice(), [State::Failed { detail }] if detail.contains("validation failed")),
        "{states:?}"
    );

    let still = Applications::stale(existing, true);
    let (states, _) = seed_applications(still, &[app("http://sonarr:8989")]).await;
    assert!(
        matches!(states.as_slice(), [State::Failed { detail }] if detail.contains("cannot connect")),
        "{states:?}"
    );
}

/// A rehearsal does not test an application the aggregator holds: the test is a `POST`.
#[tokio::test]
async fn a_rehearsal_tests_nothing() {
    let existing = vec![RegisteredApplication {
        id: "1".to_owned(),
        base_url: "http://sonarr:8989".to_owned(),
    }];
    let aggregator = Applications::stale(existing, false);
    let mut journal = Journal::new();

    let wirings = wire_applications(
        &aggregator,
        "prowlarr",
        &[app("http://sonarr:8989")],
        &mut journal,
        "t",
        true,
    )
    .await;

    assert_eq!(
        wirings.into_iter().map(|one| one.state).collect::<Vec<_>>(),
        vec![State::AlreadyWired]
    );
    assert!(aggregator.rekeyed().is_empty());
}
