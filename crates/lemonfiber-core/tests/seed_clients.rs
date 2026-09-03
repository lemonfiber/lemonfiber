//! Wiring download clients into a \\*arr, against a fake service.
//!
//! The same driver as the root folders, with the difference that is the whole
//! point of it: a client is matched by the endpoint it reaches, not by its label,
//! so one the operator renamed is recognised rather than duplicated.

mod common;

use common::service::*;

use lemonfiber_core::baseline::{Baseline, Origin, Record};
use lemonfiber_core::journal::Journal;
use lemonfiber_core::ports::service::{
    Category, ClientKind, ClientProbe, Credential, DownloadClient, RegisteredClient,
};
use lemonfiber_core::seed::{
    wholesale_drift, wire_download_clients, Baselines, Severity, State, Wiring,
};

// ---- Download clients: the same driver, matched by endpoint not label. ----

fn client(name: &str, host: &str, port: u16) -> DownloadClient {
    client_with_category(name, host, port, "tv")
}

/// A wanted client whose category lemonfiber intends to file under `category` —
/// for the drift tests, where lemonfiber's desired value is the thing that moves.
fn client_with_category(name: &str, host: &str, port: u16, category: &str) -> DownloadClient {
    DownloadClient {
        name: name.to_owned(),
        host: host.to_owned(),
        port,
        kind: ClientKind::Sabnzbd,
        credential: Credential::ApiKey("sab-key".to_owned()),
        category: Category {
            field: "tvCategory".to_owned(),
            value: category.to_owned(),
        },
    }
}

/// Run the client driver for the wanted clients, returning their resulting states
/// and the number of changes journalled. The baseline it records into is discarded
/// — the tests that assert on it drive the driver directly.
async fn seed_clients(service: FakeService, wanted: &[DownloadClient]) -> (Vec<State>, usize) {
    let mut journal = Journal::new();
    let expected = Baseline::new();
    let mut records = Baseline::new();
    let wirings = wire_download_clients(
        &service,
        "sonarr",
        wanted,
        &mut journal,
        &mut Baselines {
            expected: &expected,
            records: &mut records,
            adopt: false,
            reset: false,
        },
        "t",
    )
    .await;
    let states = wirings.into_iter().map(|wiring| wiring.state).collect();
    (states, journal.changes().len())
}

/// Run the client driver, returning the resulting states and the baseline it
/// recorded into — for the tests that assert what lemonfiber remembered it wrote.
/// The expected snapshot is empty, as on a first seed.
async fn seed_clients_recording(
    service: FakeService,
    wanted: &[DownloadClient],
) -> (Vec<State>, Baseline) {
    let mut journal = Journal::new();
    let expected = Baseline::new();
    let mut records = Baseline::new();
    let wirings = wire_download_clients(
        &service,
        "sonarr",
        wanted,
        &mut journal,
        &mut Baselines {
            expected: &expected,
            records: &mut records,
            adopt: false,
            reset: false,
        },
        "t",
    )
    .await;
    let states = wirings.into_iter().map(|wiring| wiring.state).collect();
    (states, records)
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

#[tokio::test]
async fn a_client_the_operator_re_filed_is_preserved_as_drift() {
    // lemonfiber last wrote "tv" and still wants "tv", but the operator changed the
    // category in the *arr itself. That is their edit to keep, not a mistake to
    // revert: against the baseline it reports as drift and is left exactly as it is,
    // nothing re-registered.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "qbittorrent".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: "my-own-tv".to_owned(),
        }),
    }];
    let mut journal = Journal::new();
    let mut expected = Baseline::new();
    expected.record("sonarr", "downloadclient:qbittorrent:8080", "tv", "1");
    let mut records = Baseline::new();
    let states: Vec<State> = wire_download_clients(
        &FakeService::with_clients(Mode::Normal, existing),
        "sonarr",
        &[client("qBittorrent", "qbittorrent", 8080)],
        &mut journal,
        &mut Baselines {
            expected: &expected,
            records: &mut records,
            adopt: false,
            reset: false,
        },
        "2",
    )
    .await
    .into_iter()
    .map(|wiring| wiring.state)
    .collect();
    assert_eq!(states, vec![State::Drifted]);
    assert_eq!(
        journal.changes().len(),
        0,
        "an operator's own change is preserved, not rewritten"
    );
}

/// A qBittorrent client the operator re-filed, so it reads as drift — the setup a
/// severity check reads. Recorded "tv", the service now holds "my-own-tv".
fn re_filed_client() -> Vec<RegisteredClient> {
    vec![RegisteredClient {
        id: "1".to_owned(),
        host: "qbittorrent".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: "my-own-tv".to_owned(),
        }),
    }]
}

/// Wire the wanted clients against a service holding `existing`, with lemonfiber's own
/// value recorded for the qBittorrent endpoint so a differing one reads as drift, and
/// the given test verdicts — `None` to stand in for a service that will not test.
async fn seed_clients_probed(
    existing: Vec<RegisteredClient>,
    wanted: &[DownloadClient],
    probes: Option<Vec<ClientProbe>>,
) -> Vec<Wiring> {
    let mut journal = Journal::new();
    let mut expected = Baseline::new();
    expected.record("sonarr", "downloadclient:qbittorrent:8080", "tv", "1");
    let mut records = Baseline::new();
    let base = FakeService::with_clients(Mode::Normal, existing);
    let service = match probes {
        Some(probes) => base.probing(probes),
        None => base,
    };
    wire_download_clients(
        &service,
        "sonarr",
        wanted,
        &mut journal,
        &mut Baselines {
            expected: &expected,
            records: &mut records,
            adopt: false,
            reset: false,
        },
        "2",
    )
    .await
}

/// One test verdict for a client, by id.
fn probe(id: &str, reachable: bool, detail: Option<&str>) -> ClientProbe {
    ClientProbe {
        id: id.to_owned(),
        reachable,
        detail: detail.map(str::to_owned),
    }
}

/// The breakage a wiring's warning names, or nothing where it is informational or
/// absent — the one place a severity is read, so both arms are exercised across the
/// warning and the informational tests rather than left dead in either.
fn breakage(wiring: Option<&Wiring>) -> Option<String> {
    match wiring.map(|wiring| &wiring.severity) {
        Some(Severity::Warning { breakage, .. }) => Some(breakage.clone()),
        Some(Severity::Informational) | None => None,
    }
}

/// The single wanted qBittorrent client the drift-severity tests re-file.
fn one_qbittorrent() -> [DownloadClient; 1] {
    [client("qBittorrent", "qbittorrent", 8080)]
}

#[tokio::test]
async fn a_drifted_client_the_service_cannot_reach_is_raised_to_a_warning() {
    // A category drift is the operator's own edit, ordinarily just information. But
    // the same client the service can no longer reach has broken the stack — nothing
    // downloads through it — so it is raised to a warning naming the service's own
    // words. A second, freshly-wired client alongside it is not a drift, so it is
    // never tested and stays informational.
    let wanted = [
        client("qBittorrent", "qbittorrent", 8080),
        client("SABnzbd", "sabnzbd", 8080),
    ];
    let wirings = seed_clients_probed(
        re_filed_client(),
        &wanted,
        Some(vec![probe("1", false, Some("connection refused"))]),
    )
    .await;

    assert_eq!(
        wirings.first().map(|wiring| &wiring.state),
        Some(&State::Drifted)
    );
    assert!(
        breakage(wirings.first()).is_some_and(|breakage| breakage.contains("connection refused")),
        "the unreachable drift is a warning naming the service's words"
    );
    // The freshly-wired SABnzbd client never drifted, so it was never tested.
    assert_eq!(
        wirings.get(1).map(|wiring| &wiring.state),
        Some(&State::Wired)
    );
    assert!(breakage(wirings.get(1)).is_none());
}

#[tokio::test]
async fn a_drifted_client_the_service_still_reaches_stays_informational() {
    // A drift the service can still reach has broken nothing — it is the operator's
    // edit, working — so it is left as the information it is.
    let wirings = seed_clients_probed(
        re_filed_client(),
        &one_qbittorrent(),
        Some(vec![probe("1", true, None)]),
    )
    .await;
    assert_eq!(
        wirings.first().map(|wiring| &wiring.state),
        Some(&State::Drifted)
    );
    assert!(breakage(wirings.first()).is_none());
}

#[tokio::test]
async fn an_unreachable_client_the_service_gave_no_words_for_names_a_fallback() {
    // A test that failed without the service saying why still names the breakage, so
    // the warning is never blank — a fallback stands in for the missing detail.
    let wirings = seed_clients_probed(
        re_filed_client(),
        &one_qbittorrent(),
        Some(vec![probe("1", false, None)]),
    )
    .await;
    assert!(
        breakage(wirings.first()).is_some_and(|breakage| breakage.contains("could not reach it")),
        "the warning names a fallback where the service gave no words"
    );
}

#[tokio::test]
async fn a_drift_the_service_will_not_test_stays_the_information_it_is() {
    // A service that will not run the test at all proves nothing broken, so the drift
    // is left as information rather than guessed into a warning.
    let wirings = seed_clients_probed(re_filed_client(), &one_qbittorrent(), None).await;
    assert_eq!(
        wirings.first().map(|wiring| &wiring.state),
        Some(&State::Drifted)
    );
    assert!(breakage(wirings.first()).is_none());
}

#[tokio::test]
async fn a_drift_the_test_does_not_cover_stays_informational() {
    // The service tested its clients but reported nothing for this one — no verdict is
    // not a failure, so the drift stays the information it is.
    let wirings = seed_clients_probed(
        re_filed_client(),
        &one_qbittorrent(),
        Some(vec![probe("999", false, Some("some other client"))]),
    )
    .await;
    assert!(breakage(wirings.first()).is_none());
}

/// A client the service holds under a category, for the wholesale-drift checks.
fn holding(id: &str, host: &str, port: u16, category: &str) -> RegisteredClient {
    RegisteredClient {
        id: id.to_owned(),
        host: host.to_owned(),
        port,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: category.to_owned(),
        }),
    }
}

#[test]
fn every_client_drifted_at_once_reads_as_wholesale() {
    // lemonfiber recorded "tv"; the one client the service holds now reads "shows".
    // With every managed value moved together, this is a schema change, not the
    // operator editing each by hand.
    let existing = vec![holding("1", "qbittorrent", 8080, "shows")];
    let mut expected = Baseline::new();
    expected.record("sonarr", "downloadclient:qbittorrent:8080", "tv", "1");
    let wanted = [client("qBittorrent", "qbittorrent", 8080)];
    assert!(wholesale_drift(&existing, &wanted, &expected, "sonarr"));
}

#[test]
fn one_client_still_at_lemonfibers_value_is_not_wholesale() {
    // Two clients the service holds: one drifted, one still at lemonfiber's value. Not
    // every managed value moved, so it is the operator's edits — reported as drift, not
    // re-baselined.
    let existing = vec![
        holding("1", "qbittorrent", 8080, "shows"),
        holding("2", "sabnzbd", 8080, "tv"),
    ];
    let mut expected = Baseline::new();
    expected.record("sonarr", "downloadclient:qbittorrent:8080", "tv", "1");
    expected.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "1");
    let wanted = [
        client("qBittorrent", "qbittorrent", 8080),
        client("SABnzbd", "sabnzbd", 8080),
    ];
    assert!(!wholesale_drift(&existing, &wanted, &expected, "sonarr"));
}

#[test]
fn a_service_holding_none_of_the_wanted_clients_is_not_wholesale() {
    // Nothing present drifted, so there is no wholesale drift to read — a client not
    // there yet does not, on its own, stand in for a schema change.
    let wanted = [client("qBittorrent", "qbittorrent", 8080)];
    assert!(!wholesale_drift(&[], &wanted, &Baseline::new(), "sonarr"));
}

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

/// Run the client driver against a given baseline and pass kind, returning the
/// resulting states, what this run recorded, and the number of changes journalled —
/// for the adoption tests, which turn on the baseline's origin and the adopt flag.
async fn seed_clients_with(
    service: FakeService,
    wanted: &[DownloadClient],
    expected: &Baseline,
    adopt: bool,
) -> (Vec<State>, Baseline, usize) {
    let mut journal = Journal::new();
    let mut records = Baseline::new();
    let wirings = wire_download_clients(
        &service,
        "sonarr",
        wanted,
        &mut journal,
        &mut Baselines {
            expected,
            records: &mut records,
            adopt,
            reset: false,
        },
        "2",
    )
    .await;
    let states = wirings.into_iter().map(|wiring| wiring.state).collect();
    (states, records, journal.changes().len())
}

fn a_pre_existing_client() -> Vec<RegisteredClient> {
    vec![RegisteredClient {
        id: "1".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: "their-tv".to_owned(),
        }),
    }]
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

#[tokio::test]
async fn an_adopt_pass_takes_on_a_pre_existing_unmanaged_value() {
    // The deliberate act: adopting an existing setup baselines from what is found.
    // The same pre-existing value, run through an adopt pass, is taken on — recorded
    // as the operator's own, marked adopted, with nothing written to the service.
    let (states, recorded, changes) = seed_clients_with(
        FakeService::with_clients(Mode::Normal, a_pre_existing_client()),
        &[client("SABnzbd", "sabnzbd", 8080)],
        &Baseline::new(),
        true,
    )
    .await;
    assert_eq!(states, vec![State::Adopted]);
    assert_eq!(
        changes, 0,
        "adopting what is found writes nothing to the service"
    );
    assert_eq!(
        recorded.entry("sonarr", "downloadclient:sabnzbd:8080"),
        Some(&Record {
            value: "their-tv".to_owned(),
            at: "2".to_owned(),
            origin: Origin::Adopted,
        }),
        "the operator's own value is adopted as the baseline",
    );
}

#[tokio::test]
async fn an_adopted_value_lemonfiber_also_wants_stays_adopted_not_re_recorded() {
    // The case the origin exists to guard: an adopted value that happens to equal what
    // lemonfiber would write must stay adopted, not be read as merely in-sync and taken
    // back as lemonfiber's own. It reads as adopted, and this run records nothing over
    // it — so the adopted baseline is not clobbered with a written one, which a later
    // run, once lemonfiber's desired moved, would read as stale and revert.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: "tv".to_owned(),
        }),
    }];
    let mut expected = Baseline::new();
    expected.adopt("sonarr", "downloadclient:sabnzbd:8080", "tv", "1");
    let (states, recorded, _changes) = seed_clients_with(
        FakeService::with_clients(Mode::Normal, existing),
        &[client("SABnzbd", "sabnzbd", 8080)],
        &expected,
        false,
    )
    .await;
    assert_eq!(states, vec![State::Adopted]);
    assert_eq!(
        recorded.entry("sonarr", "downloadclient:sabnzbd:8080"),
        None,
        "an adopted value equal to desired is not re-recorded as written",
    );
}

#[tokio::test]
async fn an_adopted_value_the_service_still_holds_is_kept_not_made_stale() {
    // The run after adoption: the baseline now holds the operator's value, marked
    // adopted, and the service still holds it, while lemonfiber's desired differs. A
    // written value here would be stale — lemonfiber's own, to bring up to date — but
    // an adopted one is theirs to keep, so it is left as it is and not overwritten.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: "their-tv".to_owned(),
        }),
    }];
    let mut expected = Baseline::new();
    expected.adopt("sonarr", "downloadclient:sabnzbd:8080", "their-tv", "1");
    let (states, _recorded, changes) = seed_clients_with(
        FakeService::with_clients(Mode::Normal, existing),
        &[client("SABnzbd", "sabnzbd", 8080)],
        &expected,
        false,
    )
    .await;
    assert_eq!(states, vec![State::Adopted]);
    assert_eq!(changes, 0, "an adopted value is not overwritten");
}

#[tokio::test]
async fn an_adopt_pass_promotes_a_drifted_value_and_records_it_as_adopted() {
    // lemonfiber wrote "tv" and still wants it, but the operator changed it: a normal
    // seed reports drift. An adopt pass instead promotes their edit — reporting it
    // adopted and recording what the service holds as the accepted baseline, so a
    // later seed keeps it rather than flagging it again.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: "my-own-tv".to_owned(),
        }),
    }];
    let mut expected = Baseline::new();
    expected.record("sonarr", "downloadclient:sabnzbd:8080", "tv", "1");
    let (states, recorded, changes) = seed_clients_with(
        FakeService::with_clients(Mode::Normal, existing),
        &[client("SABnzbd", "sabnzbd", 8080)],
        &expected,
        true,
    )
    .await;
    assert_eq!(states, vec![State::Adopted]);
    assert_eq!(
        changes, 0,
        "adopting an edit rewrites nothing in the service"
    );
    assert_eq!(
        recorded.entry("sonarr", "downloadclient:sabnzbd:8080"),
        Some(&Record {
            value: "my-own-tv".to_owned(),
            at: "2".to_owned(),
            origin: Origin::Adopted,
        }),
        "the operator's edit is recorded as the adopted baseline",
    );
}

#[tokio::test]
async fn a_seed_after_an_adopt_pass_keeps_the_adopted_edit() {
    // What the adopt pass recorded above, read on the next ordinary seed: the value
    // is adopted, so the seed keeps it rather than reverting to lemonfiber's default —
    // the promotion survives future seeds.
    let existing = vec![RegisteredClient {
        id: "1".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        category: Some(Category {
            field: "tvCategory".to_owned(),
            value: "my-own-tv".to_owned(),
        }),
    }];
    let mut expected = Baseline::new();
    expected.adopt("sonarr", "downloadclient:sabnzbd:8080", "my-own-tv", "2");
    let (states, _recorded, changes) = seed_clients_with(
        FakeService::with_clients(Mode::Normal, existing),
        &[client("SABnzbd", "sabnzbd", 8080)],
        &expected,
        false,
    )
    .await;
    assert_eq!(states, vec![State::Adopted]);
    assert_eq!(changes, 0, "the adopted edit is kept, not reverted");
}

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
