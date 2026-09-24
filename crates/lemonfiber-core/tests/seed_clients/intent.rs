//! Stale, conflicting and unmanaged values, and what differs only in spacing.

use crate::common::service::*;
use crate::{a_pre_existing_client, seed_clients_with};
use lemonfiber_core::baseline::{Baseline, Origin, Record};
use lemonfiber_core::journal::Journal;
use lemonfiber_core::ports::service::{Category, RegisteredClient};
use lemonfiber_core::seed::{wire_download_clients, Baselines, State};

#[tokio::test]
async fn a_client_at_lemonfibers_old_value_with_a_moved_intent_is_stale() {
    // The baseline records lemonfiber last wrote "tv"; the service still holds "tv",
    // but lemonfiber now wants "tv-hd". Only lemonfiber's intent moved, so the
    // client is lemonfiber's own value fallen behind — stale, left as it is (never
    // overwritten) and reported, not preserved as though it were an operator edit.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: "tv".to_owned(),
        }),
    }];
    let mut journal = Journal::new();
    let mut expected = Baseline::new();
    expected.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "1");
    let mut records = Baseline::new();
    let states: Vec<State> = wire_download_clients(
        &FakeService::with_clients(Mode::Normal, existing),
        "sonarr",
        &[client_with_category("SABnzbd", "sabnzbd", 8080, "tv-hd")],
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
    assert_eq!(states, vec![State::Stale]);
    assert_eq!(
        journal.changes().len(),
        0,
        "a stale value is not overwritten"
    );
}

#[tokio::test]
async fn a_client_both_sides_changed_is_a_conflict() {
    // The baseline records "tv"; the operator re-filed to "mine" and lemonfiber now
    // wants "tv-hd". Both moved away from the baseline, so lemonfiber presents the
    // conflict and leaves the value — it does not resolve it on its own.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: "mine".to_owned(),
        }),
    }];
    let mut journal = Journal::new();
    let mut expected = Baseline::new();
    expected.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "1");
    let mut records = Baseline::new();
    let states: Vec<State> = wire_download_clients(
        &FakeService::with_clients(Mode::Normal, existing),
        "sonarr",
        &[client_with_category("SABnzbd", "sabnzbd", 8080, "tv-hd")],
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
    // The conflict is presented with both sides — what the operator set beside what
    // lemonfiber would write — so the operator can see the clash, and nothing is
    // written: presenting is not resolving.
    assert_eq!(
        states,
        vec![State::Conflicted {
            yours: Some("mine".to_owned()),
            ours: "tv-hd".to_owned(),
        }]
    );
    assert_eq!(journal.changes().len(), 0, "a conflict is not resolved");
}

#[tokio::test]
async fn a_category_differing_only_by_whitespace_is_not_drift() {
    // lemonfiber wrote "tv" and still wants it; the service reports it back with
    // surrounding whitespace lemonfiber's own value does not carry — the kind of
    // difference a normalisation on write leaves. Compared by canonical form the two
    // are the same category, so it reads as already wired, not as drift to preserve.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: " tv ".to_owned(),
        }),
    }];
    let mut expected = Baseline::new();
    expected.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "1");
    let (states, _recorded, changes) = seed_clients_with(
        FakeService::with_clients(Mode::Normal, existing),
        &[client("SABnzbd", "sabnzbd", 8080)],
        &expected,
        false,
    )
    .await;
    assert_eq!(states, vec![State::AlreadyWired]);
    assert_eq!(
        changes, 0,
        "a value the same but for whitespace is not written again"
    );
}

#[tokio::test]
async fn a_whitespace_only_difference_with_no_baseline_is_lemonfibers_own() {
    // No baseline, and the service holds what lemonfiber would write but for
    // surrounding whitespace. Canonically the two are the same, so this is not the
    // operator's own unmanaged value — it is lemonfiber's, already in place: already
    // wired, and recorded as written, not adopted.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: " tv ".to_owned(),
        }),
    }];
    let (states, recorded, _changes) = seed_clients_with(
        FakeService::with_clients(Mode::Normal, existing),
        &[client("SABnzbd", "sabnzbd", 8080)],
        &Baseline::new(),
        false,
    )
    .await;
    assert_eq!(states, vec![State::AlreadyWired]);
    assert_eq!(
        recorded.entry("sonarr", "downloadclient:sabnzbd:8080"),
        Some(&Record {
            value: "tv".to_owned(),
            at: "2".to_owned(),
            origin: Origin::Written,
        }),
        "the value is lemonfiber's own, recorded as written not adopted",
    );
}

#[tokio::test]
async fn a_pre_existing_value_with_no_baseline_is_reported_unmanaged_not_drift() {
    // A service already configured before lemonfiber managed it: it holds a value
    // lemonfiber never wrote, and there is no baseline. An ordinary seed reports it
    // as unmanaged — the operator's own, outside lemonfiber's scope — rather than as
    // mass drift, and records nothing: it does not claim the value as lemonfiber's on
    // its own, so a lost baseline is never silently frozen.
    let (states, recorded, changes) = seed_clients_with(
        FakeService::with_clients(Mode::Normal, a_pre_existing_client()),
        &[client("SABnzbd", "sabnzbd", 8080)],
        &Baseline::new(),
        false,
    )
    .await;
    assert_eq!(states, vec![State::Unmanaged]);
    assert_eq!(
        changes, 0,
        "an unmanaged value is not written to the service"
    );
    assert_eq!(
        recorded.entry("sonarr", "downloadclient:sabnzbd:8080"),
        None,
        "an ordinary seed does not adopt a pre-existing value on its own",
    );
}
