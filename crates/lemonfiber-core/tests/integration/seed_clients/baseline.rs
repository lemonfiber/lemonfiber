//! What lemonfiber records as the value it expects.

use super::common::service::*;
use super::seed_clients_recording;
use lemonfiber_core::baseline::Baseline;
use lemonfiber_core::journal::Journal;
use lemonfiber_core::ports::service::{Category, RegisteredClient};
use lemonfiber_core::seed::{wire_download_clients, Baselines, State};

#[tokio::test]
async fn a_wired_client_records_its_category_as_the_expected_baseline() {
    // What lemonfiber writes it remembers: the category is recorded, keyed by the
    // client's endpoint, so a later run can tell an operator's re-filing from
    // lemonfiber's own value.
    let (states, baseline) = seed_clients_recording(
        FakeService::with_clients(Mode::Normal, Vec::new()),
        &[client("SABnzbd", "sabnzbd", 8080)],
    )
    .await;
    assert_eq!(states, vec![State::Wired]);
    assert_eq!(
        baseline.expected("sonarr", "downloadclient:sabnzbd:8080"),
        Some("tv"),
    );
}

#[tokio::test]
async fn a_client_already_at_lemonfibers_value_is_recorded_as_the_baseline_too() {
    // An already-correct client was not written this run, but it is lemonfiber's
    // value, so it is recorded as expected — which is also how a lost baseline
    // re-forms from what already matches lemonfiber's intent.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: "tv".to_owned(),
        }),
    }];
    let (states, baseline) = seed_clients_recording(
        FakeService::with_clients(Mode::Normal, existing),
        &[client("SABnzbd", "sabnzbd", 8080)],
    )
    .await;
    assert_eq!(states, vec![State::AlreadyWired]);
    assert_eq!(
        baseline.expected("sonarr", "downloadclient:sabnzbd:8080"),
        Some("tv"),
    );
}

#[tokio::test]
async fn an_operators_re_filed_client_is_not_recorded_as_the_baseline() {
    // A drifted client is the operator's edit, not lemonfiber's value, so its
    // category is not recorded as expected: the baseline keeps what lemonfiber last
    // wrote — here "tv" — which is what lets a later run read the difference as
    // drift rather than as lemonfiber's own intent.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: "my-own-tv".to_owned(),
        }),
    }];
    let mut journal = Journal::new();
    let mut expected = Baseline::new();
    expected.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "1");
    let mut records = Baseline::new();
    let states: Vec<State> = wire_download_clients(
        &FakeService::with_clients(Mode::Normal, existing),
        "sonarr",
        &[client("SABnzbd", "sabnzbd", 8080)],
        &mut journal,
        &mut Baselines {
            expected: &expected,
            records: &mut records,
            adopt: false,
            reset: false,
            rehearsing: false,
        },
        "2",
    )
    .await
    .into_iter()
    .map(|wiring| wiring.state)
    .collect();
    let baseline = records;
    assert_eq!(states, vec![State::Drifted]);
    assert_eq!(
        baseline.expected("sonarr", "downloadclient:sabnzbd:8080"),
        None,
    );
}
