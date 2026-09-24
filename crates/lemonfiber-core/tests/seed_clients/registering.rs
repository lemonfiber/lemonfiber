//! A download client registered, read back and recorded.

use crate::common::service::*;
use crate::seed_clients;
use lemonfiber_core::baseline::Baseline;
use lemonfiber_core::journal::Journal;
use lemonfiber_core::ports::service::{Category, DownloadClient, RegisteredClient};
use lemonfiber_core::seed::{wire_download_clients, Baselines, State};

/// Run the client driver as a rehearsal: the same pass over the same service, with
/// the registering left out. `expected` is what lemonfiber last recorded, and `adopt`
/// is whether this is the pass that promotes an operator's value rather than the one
/// that pushes lemonfiber's.
async fn would_seed_clients(
    service: &FakeService,
    wanted: &[DownloadClient],
    expected: &Baseline,
    adopt: bool,
) -> (Vec<State>, usize) {
    let mut journal = Journal::new();
    let mut records = Baseline::new();
    let wirings = wire_download_clients(
        service,
        "sonarr",
        wanted,
        &mut journal,
        &mut Baselines {
            expected,
            records: &mut records,
            adopt,
            reset: false,
            rehearsing: true,
        },
        "t",
    )
    .await;
    let states = wirings.into_iter().map(|wiring| wiring.state).collect();
    (states, journal.changes().len())
}

/// A rehearsal names the client it would register and the category it would file it
/// under, and registers none of it.
///
/// The two halves together are the whole requirement: a report saying a connection
/// would be made without saying what it would be made *to* is a count, and a count is
/// what an operator asking what a run would do already has.
#[tokio::test]
async fn a_rehearsed_pass_names_what_it_would_push_and_pushes_none_of_it() {
    let service = FakeService::with_clients(Mode::Normal, Vec::new());
    let (states, recorded) = would_seed_clients(
        &service,
        &[client("SABnzbd", "sabnzbd", 8080)],
        &Baseline::new(),
        false,
    )
    .await;

    assert_eq!(
        states,
        vec![State::WouldWire {
            yours: None,
            ours: Some("tv".to_owned()),
        }]
    );
    assert_eq!(
        recorded, 0,
        "nothing was registered, so there was nothing to journal"
    );
}

/// A rehearsal of an adopt pass says which value would be taken on, and does not say
/// what it is — the rule an unmanaged value is already reported under, so a secret
/// among the adopted is never put on display by a question.
#[tokio::test]
async fn a_rehearsed_adopt_names_the_connection_and_never_the_value() {
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
    let mut expected = Baseline::new();
    expected.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "1");

    let (states, recorded) = would_seed_clients(
        &service,
        &[client("SABnzbd", "sabnzbd", 8080)],
        &expected,
        true,
    )
    .await;

    assert_eq!(states, vec![State::WouldAdopt]);
    assert_eq!(recorded, 0, "an adopt writes to no service on any run");
}

#[tokio::test]
async fn an_absent_download_client_is_registered_read_back_and_recorded() {
    let (states, recorded) = seed_clients(
        FakeService::with_clients(Mode::Normal, Vec::new()),
        &[client("SABnzbd", "sabnzbd", 8080)],
    )
    .await;
    assert_eq!(states, vec![State::Wired]);
    assert_eq!(recorded, 1, "the write is journalled so it can be undone");
}

#[tokio::test]
async fn a_client_at_the_same_endpoint_is_left_untouched_despite_a_different_name() {
    // The connection detail, not the label, decides identity: the operator
    // renamed the client, but it reaches the same host and port, so it is left
    // alone rather than registered a second time.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "qbittorrent".to_owned(),
        port: 8080,
        // Same category as wanted, so only the name differs.
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: "tv".to_owned(),
        }),
    }];
    let (states, recorded) = seed_clients(
        FakeService::with_clients(Mode::Normal, existing),
        &[client("qBittorrent — my own name", "qbittorrent", 8080)],
    )
    .await;
    assert_eq!(states, vec![State::AlreadyWired]);
    assert_eq!(
        recorded, 0,
        "a client already at the endpoint is not duplicated"
    );
}
