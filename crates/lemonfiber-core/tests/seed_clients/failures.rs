//! A service that is down, refuses, or loses a write.

use crate::common::service::*;
use crate::seed_clients;
use lemonfiber_core::seed::State;

#[tokio::test]
async fn an_unavailable_service_skips_every_download_client() {
    let (states, recorded) = seed_clients(
        FakeService::with_clients(Mode::Down, Vec::new()),
        &[
            client("SABnzbd", "sabnzbd", 8080),
            client("qBittorrent", "qbittorrent", 8080),
        ],
    )
    .await;
    // Counted before it is judged: `all` over an empty list is true, so a run that
    // produced no state at all would pass a test named for skipping every client
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
async fn a_service_that_refuses_the_client_listing_fails() {
    let (states, _) = seed_clients(
        FakeService::with_clients(Mode::RefusesList, Vec::new()),
        &[client("SABnzbd", "sabnzbd", 8080)],
    )
    .await;
    assert!(
        matches!(states.as_slice(), [State::Failed { .. }]),
        "{states:?}"
    );
}

#[tokio::test]
async fn a_rejected_client_registration_fails_with_the_services_own_words() {
    let (states, recorded) = seed_clients(
        FakeService::with_clients(Mode::RejectsRegister, Vec::new()),
        &[client("SABnzbd", "sabnzbd", 8080)],
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
async fn a_client_write_that_does_not_appear_when_read_back_is_a_failure() {
    // Accepted but not reported back, so it did not land — not done, not recorded.
    let (states, recorded) = seed_clients(
        FakeService::with_clients(Mode::Swallows, Vec::new()),
        &[client("SABnzbd", "sabnzbd", 8080)],
    )
    .await;
    assert!(
        matches!(states.as_slice(), [State::Failed { .. }]),
        "{states:?}"
    );
    assert_eq!(recorded, 0);
}

#[tokio::test]
async fn a_service_that_stops_answering_after_the_client_write_is_skipped() {
    // The write went out but could not be confirmed, so it is left for a later
    // run to reconcile rather than declared wired.
    let (states, recorded) = seed_clients(
        FakeService::with_clients(Mode::DropsAfterRegister, Vec::new()),
        &[client("SABnzbd", "sabnzbd", 8080)],
    )
    .await;
    assert!(
        matches!(states.as_slice(), [State::Skipped { .. }]),
        "{states:?}"
    );
    assert_eq!(recorded, 0, "an unconfirmed write is not recorded as done");
}
