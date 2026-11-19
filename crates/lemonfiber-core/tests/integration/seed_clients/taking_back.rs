//! A download client lemonfiber created, taken back once nothing it points at fills the
//! curator's download ask, and every other client left where it is.

use super::common::service::*;
use lemonfiber_core::baseline::Baseline;
use lemonfiber_core::instant::Instant;
use lemonfiber_core::journal::Journal;
use lemonfiber_core::ports::service::{Client as _, DownloadClient, RegisteredClient};
use lemonfiber_core::seed::{
    folded, take_back_clients, wire_download_clients, Baselines, Severity, State, Wiring,
};

/// qBittorrent, as the stack declares it.
fn qbittorrent() -> DownloadClient {
    client("qBittorrent", "qbittorrent", 8080)
}

/// Transmission, as the stack declares it.
fn transmission() -> DownloadClient {
    client("Transmission", "transmission", 9091)
}

/// Where `clients` answer.
fn at(clients: &[DownloadClient]) -> Vec<(String, u16)> {
    clients
        .iter()
        .map(|one| (one.host.clone(), one.port))
        .collect()
}

/// One seed of `curator` as the pass runs it: its clients wired, what it created kept,
/// and what no filler answers as any more taken back, all folded into `baseline`.
async fn seeded(
    curator: &Receiving,
    baseline: &mut Baseline,
    wanted: &[DownloadClient],
) -> Vec<Wiring> {
    let mut records = Baseline::new();
    let mut wirings = wire_download_clients(
        curator,
        "radarr",
        wanted,
        &mut Journal::new(),
        &mut Baselines {
            expected: baseline,
            records: &mut records,
            adopt: false,
            reset: false,
            rehearsing: false,
        },
        Instant::from_seconds(1),
    )
    .await;
    let taken = take_back_clients(curator, "radarr", baseline, &at(wanted), false).await;
    wirings.extend(taken.wirings);
    folded(baseline, "radarr", &records, &taken.let_go);
    wirings
}

/// What `curator` holds, by host.
async fn holding(curator: &Receiving) -> Vec<String> {
    curator
        .download_clients()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|one| one.host)
        .collect()
}

/// The state of the wiring named `connection`.
fn state_of<'a>(wirings: &'a [Wiring], connection: &str) -> Option<&'a State> {
    wirings
        .iter()
        .find(|wiring| wiring.connection == connection)
        .map(|wiring| &wiring.state)
}

#[tokio::test]
async fn a_client_swapped_out_is_taken_back_and_swapped_in_again_the_other_is() {
    let curator = Receiving::with(Mode::Normal, Vec::new());
    let mut baseline = Baseline::new();

    seeded(&curator, &mut baseline, &[qbittorrent()]).await;
    assert_eq!(holding(&curator).await, ["qbittorrent"]);

    let swapped = seeded(&curator, &mut baseline, &[transmission()]).await;
    assert_eq!(
        state_of(&swapped, "Transmission into radarr"),
        Some(&State::Wired)
    );
    assert_eq!(
        state_of(&swapped, "qBittorrent into radarr"),
        Some(&State::TakenBack)
    );
    assert_eq!(holding(&curator).await, ["transmission"]);

    let back = seeded(&curator, &mut baseline, &[qbittorrent()]).await;
    assert_eq!(
        state_of(&back, "qBittorrent into radarr"),
        Some(&State::Wired)
    );
    assert_eq!(
        state_of(&back, "Transmission into radarr"),
        Some(&State::TakenBack)
    );
    assert_eq!(holding(&curator).await, ["qbittorrent"]);

    let again = seeded(&curator, &mut baseline, &[qbittorrent()]).await;
    assert_eq!(
        again.iter().map(|wiring| &wiring.state).collect::<Vec<_>>(),
        [&State::AlreadyWired],
        "nothing is taken back twice"
    );
}

#[tokio::test]
async fn a_client_the_operator_added_or_lemonfiber_adopted_is_never_taken_back() {
    let theirs = |id: &str, host: &str| RegisteredClient {
        id: id.to_owned(),
        host: host.to_owned(),
        port: 8080,
        category: None,
    };
    let curator = Receiving::with_clients(
        Mode::Normal,
        vec![theirs("7", "nzbget"), theirs("8", "qbittorrent")],
    );
    let mut baseline = Baseline::new();
    let mut records = Baseline::new();
    wire_download_clients(
        &curator,
        "radarr",
        &[qbittorrent()],
        &mut Journal::new(),
        &mut Baselines {
            expected: &Baseline::new(),
            records: &mut records,
            adopt: true,
            reset: false,
            rehearsing: false,
        },
        Instant::from_seconds(1),
    )
    .await;
    baseline.merge(&records);

    let wirings = seeded(&curator, &mut baseline, &[transmission()]).await;

    assert_eq!(state_of(&wirings, "qBittorrent into radarr"), None);
    assert_eq!(
        holding(&curator).await,
        ["nzbget", "qbittorrent", "transmission"]
    );
}

#[tokio::test]
async fn a_client_still_declared_where_it_points_is_kept() {
    let curator = Receiving::with(Mode::Normal, Vec::new());
    let mut baseline = Baseline::new();
    seeded(&curator, &mut baseline, &[qbittorrent()]).await;

    let taken =
        take_back_clients(&curator, "radarr", &baseline, &at(&[qbittorrent()]), false).await;

    assert!(taken.wirings.is_empty() && taken.let_go.is_empty());
    assert_eq!(holding(&curator).await, ["qbittorrent"]);
}

#[tokio::test]
async fn a_client_the_operator_pointed_elsewhere_or_removed_is_let_go_of_untouched() {
    let curator = Receiving::with(Mode::Normal, Vec::new());
    let mut baseline = Baseline::new();
    seeded(&curator, &mut baseline, &[qbittorrent()]).await;
    let pointed = Receiving::with_clients(
        Mode::Normal,
        vec![RegisteredClient {
            id: "100".to_owned(),
            host: "a-seedbox".to_owned(),
            port: 8080,
            category: None,
        }],
    );
    let removed = Receiving::with(Mode::Normal, Vec::new());

    for elsewhere in [&pointed, &removed] {
        let taken = take_back_clients(elsewhere, "radarr", &baseline, &[], false).await;
        assert!(taken.wirings.is_empty());
        assert_eq!(taken.let_go, ["made-downloadclient:100"]);
    }
    assert_eq!(holding(&pointed).await, ["a-seedbox"]);
}

#[tokio::test]
async fn a_rehearsal_says_what_it_would_take_back_and_takes_nothing() {
    let curator = Receiving::with(Mode::Normal, Vec::new());
    let mut baseline = Baseline::new();
    seeded(&curator, &mut baseline, &[qbittorrent()]).await;

    let taken = take_back_clients(&curator, "radarr", &baseline, &[], true).await;

    assert_eq!(
        state_of(&taken.wirings, "qBittorrent into radarr"),
        Some(&State::WouldTakeBack)
    );
    assert!(taken.let_go.is_empty());
    assert_eq!(holding(&curator).await, ["qbittorrent"]);
}

#[tokio::test]
async fn a_curator_that_cannot_or_will_not_let_a_client_go_keeps_it_and_says_so() {
    let mut baseline = Baseline::new();
    seeded(
        &Receiving::with(Mode::Normal, Vec::new()),
        &mut baseline,
        &[qbittorrent()],
    )
    .await;
    let held = || {
        vec![RegisteredClient {
            id: "100".to_owned(),
            host: "qbittorrent".to_owned(),
            port: 8080,
            category: None,
        }]
    };

    let keeping = Receiving::with_clients(Mode::KeepsClients, held());
    let kept = take_back_clients(&keeping, "radarr", &baseline, &[], false).await;
    let pinning = Receiving::with_clients(Mode::PinsClients, held());
    let pinned = take_back_clients(&pinning, "radarr", &baseline, &[], false).await;

    assert!(matches!(
        kept.wirings.first(),
        Some(Wiring { state: State::Skipped { .. }, severity: Severity::Warning { breakage, .. }, .. })
            if breakage.contains("qBittorrent")
    ));
    assert!(matches!(
        pinned.wirings.first().map(|wiring| &wiring.state),
        Some(State::Failed { detail }) if detail.contains("HTTP 409") && detail.contains("Nyaa")
    ));
    assert!(kept.let_go.is_empty() && pinned.let_go.is_empty());
    assert_eq!(holding(&keeping).await, ["qbittorrent"]);
}

#[tokio::test]
async fn a_curator_not_answering_has_nothing_taken_back() {
    let mut baseline = Baseline::new();
    seeded(
        &Receiving::with(Mode::Normal, Vec::new()),
        &mut baseline,
        &[qbittorrent()],
    )
    .await;

    let taken = take_back_clients(
        &Receiving::with(Mode::Down, Vec::new()),
        "radarr",
        &baseline,
        &[],
        false,
    )
    .await;

    assert!(taken.wirings.is_empty() && taken.let_go.is_empty());
}
