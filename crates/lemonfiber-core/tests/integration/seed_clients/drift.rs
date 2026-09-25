//! A client the operator changed, told apart from one that is broken.

use super::common::service::*;
use lemonfiber_core::baseline::Baseline;
use lemonfiber_core::journal::Journal;
use lemonfiber_core::ports::service::{Category, ClientProbe, DownloadClient, RegisteredClient};
use lemonfiber_core::seed::{wire_download_clients, Baselines, Severity, State, Wiring};

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
            rehearsing: false,
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
            rehearsing: false,
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
