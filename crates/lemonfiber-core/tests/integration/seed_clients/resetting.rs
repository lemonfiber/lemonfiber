//! Putting a drifted category back.

use super::common::service::*;
use super::seed_clients_recording;
use lemonfiber_core::baseline::Baseline;
use lemonfiber_core::journal::Journal;
use lemonfiber_core::ports::service::{Category, RegisteredClient};
use lemonfiber_core::seed::{wire_download_clients, Baselines, State};

#[tokio::test]
async fn a_reset_reverts_a_drifted_category_to_lemonfibers() {
    // The same drift as above — the operator changed the category — but a reset writes
    // lemonfiber's own back over it: the client is updated in place, the connection reads
    // wired again, and the reverted value is recorded so the drift is gone.
    let service = FakeService::with_clients(
        Mode::Normal,
        vec![RegisteredClient {
            id: "1".to_owned(),
            host: "sabnzbd".to_owned(),
            port: 8080,
            category: Some(Category {
                field: "tvCategory".to_owned(),
                value: "my-own-tv".to_owned(),
            }),
        }],
    );
    let mut journal = Journal::new();
    let mut expected = Baseline::new();
    expected.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "1");
    let mut records = Baseline::new();
    let states: Vec<State> = wire_download_clients(
        &service,
        "sonarr",
        &[client("SABnzbd", "sabnzbd", 8080)],
        &mut journal,
        &mut Baselines {
            expected: &expected,
            records: &mut records,
            adopt: false,
            reset: true,
            rehearsing: false,
        },
        "2",
    )
    .await
    .into_iter()
    .map(|wiring| wiring.state)
    .collect();
    assert_eq!(states, vec![State::Wired], "the drift is reverted to wired");
    // lemonfiber's value is recorded, so a later run reads no drift.
    assert_eq!(
        records.expected("sonarr", "downloadclient:sabnzbd:8080"),
        Some("tv"),
    );
}

#[tokio::test]
async fn a_reset_a_service_refuses_is_reported_as_failed_not_recorded() {
    // A reset whose in-place update the service will not take leaves the drift reported
    // as a failure rather than falsely recorded as reverted.
    let service = FakeService::with_clients(
        Mode::RefusesUpdate,
        vec![RegisteredClient {
            id: "1".to_owned(),
            host: "sabnzbd".to_owned(),
            port: 8080,
            category: Some(Category {
                field: "tvCategory".to_owned(),
                value: "my-own-tv".to_owned(),
            }),
        }],
    );
    let mut journal = Journal::new();
    let mut expected = Baseline::new();
    expected.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "1");
    let mut records = Baseline::new();
    let wirings = wire_download_clients(
        &service,
        "sonarr",
        &[client("SABnzbd", "sabnzbd", 8080)],
        &mut journal,
        &mut Baselines {
            expected: &expected,
            records: &mut records,
            adopt: false,
            reset: true,
            rehearsing: false,
        },
        "2",
    )
    .await;
    assert!(matches!(
        wirings.first().map(|wiring| &wiring.state),
        Some(State::Failed { .. })
    ));
    assert_eq!(
        records.expected("sonarr", "downloadclient:sabnzbd:8080"),
        None
    );
}

#[tokio::test]
async fn a_reset_registers_nothing_a_preview_did_not_show() {
    // A reset reverts drift and only drift. A client the service does not hold — never
    // registered, so absent — is not a drift to revert, so a reset leaves it: it is not
    // registered, not reported as wired, and not recorded. A confirmed reset must do no
    // more than its preview showed, which listed no absent connection.
    let service = FakeService::with_clients(Mode::Normal, Vec::new());
    let mut journal = Journal::new();
    let mut records = Baseline::new();
    let wirings = wire_download_clients(
        &service,
        "sonarr",
        &[client("SABnzbd", "sabnzbd", 8080)],
        &mut journal,
        &mut Baselines {
            expected: &Baseline::new(),
            records: &mut records,
            adopt: false,
            reset: true,
            rehearsing: false,
        },
        "2",
    )
    .await;
    assert!(
        !wirings
            .iter()
            .any(|wiring| matches!(wiring.state, State::Wired)),
        "an absent client is never registered by a reset"
    );
    assert_eq!(
        journal.changes().len(),
        0,
        "a reset writes nothing for a client that was never there"
    );
    assert_eq!(
        records.expected("sonarr", "downloadclient:sabnzbd:8080"),
        None,
        "a reset records nothing for a client it did not touch"
    );
}

#[tokio::test]
async fn a_categoryless_client_lemonfiber_never_wrote_is_left_and_not_recorded() {
    // The service holds a client at the endpoint but reports no category, and there
    // is no baseline — lemonfiber never wrote it. With nothing to judge against it is
    // the operator's own, pre-existing and unmanaged, left as it is; and with no
    // value to adopt (the client is categoryless) nothing is recorded, so a later run
    // reads the operator's eventual category as their own value, not a conflict
    // against a baseline lemonfiber never set.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        category: None,
    }];
    let (states, baseline) = seed_clients_recording(
        FakeService::with_clients(Mode::Normal, existing),
        &[client("SABnzbd", "sabnzbd", 8080)],
    )
    .await;
    assert_eq!(states, vec![State::Unmanaged]);
    assert_eq!(
        baseline.expected("sonarr", "downloadclient:sabnzbd:8080"),
        None,
    );
}
