use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};

use lemonfiber_contract::capabilities::download::{torrent, usenet};
use lemonfiber_contract::Contracted;
use lemonfiber_fixtures::scratch::Scratch;

use super::{
    declared_downloads, download_targets, read_transfers, torrent_client, DownloadTarget,
    Downloading,
};
use crate::config::Settings;
use crate::dashboard::Protocol;
use crate::ports::service::{Download, Filed, Seeded, UsenetAccount};
use crate::test_support::{
    a_context, a_placed, an_installed, contracted, contracted_context, env_without_password,
    first_party, json, stack, CONTRACTED_KEY,
};
use crate::wiring::{Chosen, Fillers};

/// A client naming the stack's torrent client's API, as a plugin's service names it.
const fn torrent_api() -> lemonfiber_manifest::Api {
    lemonfiber_manifest::Api {
        kind: lemonfiber_manifest::ApiKind::Qbittorrent,
        key_source: lemonfiber_manifest::KeySource::ConfigIni,
        path: None,
        version: None,
    }
}

/// The embedded stack beside a plugin's client naming the stack's torrent client's API
/// and speaking no contract, published on the host.
fn beside_an_unadapted_client() -> Fillers {
    let client = a_placed(
        "seedbox",
        &["download.torrent"],
        Some(torrent_api()),
        Some(9091),
    );
    stack()
        .manifest()
        .map(|manifest| {
            Fillers::of(
                &manifest,
                &[an_installed("seeding", vec![client])],
                &Chosen::default(),
                None,
            )
        })
        .unwrap_or_default()
}

/// A plugin's client naming the torrent client's API and speaking no contract is declared
/// with nothing to reach it by, the stack's kept clients naming no API are not declared at
/// all, and nothing is asked of any.
#[tokio::test]
async fn a_client_no_adapter_fronts_is_declared_and_never_reached() {
    let fillers = beside_an_unadapted_client();
    let http = Fake::always(Answer::reply(200, "[]"));
    let ctx = a_context().build().with_http(http.clone());

    let declared = declared_downloads(&ctx, &fillers).await;
    let unreached: Vec<&str> = declared
        .iter()
        .filter(|client| client.target.is_none())
        .map(|client| client.id.as_str())
        .collect();

    assert_eq!(unreached, ["seedbox"]);
    assert!(download_targets(&ctx, &fillers).await.is_empty());
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

/// A record of what is installed that will not read narrows a host read to the stack's
/// own services rather than failing it.
#[test]
fn an_unreadable_register_leaves_the_stacks_own_services() {
    let env = env_without_password("host-unread");
    let beside = env.with_file_name(crate::config::paths::PLUGINS);
    assert!(std::fs::write(&beside, "not a register").is_ok());
    let ctx = a_context()
        .settings(Settings {
            env_file: Some(env),
            ..Settings::default()
        })
        .build();

    let manifests: Vec<_> = ctx.stack.manifest().into_iter().collect();
    let fillers: Vec<_> = manifests
        .iter()
        .map(|manifest| super::host_fillers(&ctx, manifest, None))
        .collect();

    assert_eq!(fillers.len(), 1, "the embedded stack parses");
    assert!(fillers.iter().all(|fillers| {
        fillers.service("qbittorrent").is_some()
            && fillers
                .services()
                .all(|one| one.origin == crate::origin::Origin::Bundled)
    }));
}

/// The torrent client is the first target that is one, whatever comes before it: a
/// Usenet client declared first is passed over rather than taken for it.
#[test]
fn a_usenet_client_declared_first_is_not_taken_for_the_torrent_client() {
    let adapter = |service: &str| {
        Contracted::new(Fake::silent(), "http://127.0.0.1:8080", service, "the-key")
    };
    let targets = [
        DownloadTarget {
            service: "usenet".to_owned(),
            protocol: Protocol::Usenet,
            adapter: adapter("usenet"),
        },
        DownloadTarget {
            service: "torrent".to_owned(),
            protocol: Protocol::Torrent,
            adapter: adapter("torrent"),
        },
    ];

    assert!(torrent_client(&targets[..1]).is_none());
    assert!(torrent_client(&targets).is_some());
}

/// The plugin service that speaks a download client's contract.
const FETCHER: &str = "fetcher";

/// Where the contracted client answers on the host, for `capability`.
fn contract_base(capability: &str) -> String {
    format!("http://127.0.0.1:8080/lemonfiber/{capability}/v1/")
}

/// What the contracted client says it is downloading.
fn arriving() -> Download {
    Download {
        name: "Arriving".to_owned(),
        progress: 40,
        speed: Some(9),
        eta: None,
        remaining: Some(600),
    }
}

/// A first-party plugin's service speaking `capability`, holding its key where `keyed`,
/// beside the stack's own clients.
fn speaking(
    name: &str,
    capability: &str,
    keyed: bool,
    http: Arc<Fake>,
) -> (Scratch, crate::app::Ctx, Fillers) {
    let project = Scratch::new(name);
    let installed = contracted("downloading", FETCHER, capability);
    let fillers = stack()
        .manifest()
        .map(|manifest| {
            Fillers::trusting(
                &manifest,
                &[installed],
                &Chosen::default(),
                Some(&project),
                &first_party("downloading"),
            )
        })
        .unwrap_or_default();
    let ctx = contracted_context(&project, FETCHER, keyed)
        .build()
        .with_http(http);
    (project, ctx, fillers)
}

/// The targets among `targets` that are the contracted client.
fn fetching(targets: Vec<DownloadTarget>) -> Vec<DownloadTarget> {
    targets
        .into_iter()
        .filter(|target| target.service == FETCHER)
        .collect()
}

/// Every request `http` was sent on the contracted client's port, each with the key it
/// was sent under.
fn to_the_fetcher(http: &Fake) -> Vec<(String, Option<String>)> {
    http.requests()
        .into_iter()
        .filter(|asked| asked.url.starts_with("http://127.0.0.1:8080/"))
        .map(|asked| {
            let key = asked
                .headers
                .into_iter()
                .find(|(name, _)| name == "Authorization")
                .map(|(_, value)| value);
            (asked.url, key)
        })
        .collect()
}

/// Whether every request in `asked` went to `capability`'s contract under the client's
/// own key.
fn over_the_contract(asked: &[(String, Option<String>)], capability: &str) -> bool {
    let base = contract_base(capability);
    let key = format!("Bearer {CONTRACTED_KEY}");
    asked
        .iter()
        .all(|(url, sent)| url.starts_with(&base) && sent.as_deref() == Some(key.as_str()))
}

#[tokio::test]
async fn a_torrent_client_speaking_its_contract_is_read_over_it_with_its_own_key() {
    let held = Seeded {
        name: "Imported".to_owned(),
        bytes: 8_000,
        ratio: 175,
    };
    let http = Fake::by_path(vec![
        (
            "download.torrent/v1/transfers",
            Answer::reply(200, json(&vec![arriving()])),
        ),
        (
            "download.torrent/v1/seeding",
            Answer::reply(200, json(&vec![held.clone()])),
        ),
    ]);
    let (_project, ctx, fillers) = speaking(
        "downloads-contracted-torrent",
        torrent::CAPABILITY,
        true,
        http.clone(),
    );

    let targets = fetching(download_targets(&ctx, &fillers).await);
    assert!(
        matches!(
            targets.as_slice(),
            [one] if one.protocol == Protocol::Torrent
        ),
        "asked over the torrent contract"
    );
    let mut read = Vec::new();
    for target in &targets {
        read.extend(read_transfers(target).await);
    }
    assert_eq!(read, vec![arriving()]);
    let seeding = match torrent_client(&targets) {
        Some(holder) => holder.seeding().await.ok(),
        None => None,
    };
    assert_eq!(seeding, Some(vec![held]));

    let asked = to_the_fetcher(&http);
    assert_eq!(asked.len(), 2, "{asked:?}");
    assert!(over_the_contract(&asked, torrent::CAPABILITY), "{asked:?}");
}

#[tokio::test]
async fn a_usenet_client_speaking_its_contract_is_read_over_it_with_its_own_key() {
    let account = UsenetAccount {
        name: "news.example".to_owned(),
        enabled: true,
        quota: None,
        downloaded: 2_000,
        daily: Vec::new(),
        expires_on: None,
        standing: None,
    };
    let http = Fake::by_path(vec![
        (
            "download.usenet/v1/transfers",
            Answer::reply(200, json(&vec![arriving()])),
        ),
        (
            "download.usenet/v1/accounts",
            Answer::reply(200, json(&vec![account.clone()])),
        ),
    ]);
    let (_project, ctx, fillers) = speaking(
        "downloads-contracted-usenet",
        usenet::CAPABILITY,
        true,
        http.clone(),
    );

    let targets = fetching(download_targets(&ctx, &fillers).await);
    assert!(matches!(
        targets.as_slice(),
        [one] if one.protocol == Protocol::Usenet
    ));
    let mut read = Vec::new();
    for target in &targets {
        read.extend(read_transfers(target).await);
    }
    assert_eq!(read, vec![arriving()]);
    assert!(torrent_client(&targets).is_none());
    let accounts = match crate::app::targets::usenet_client(&ctx, &fillers).await {
        Some(client) => client.accounts().await.ok(),
        None => None,
    };
    assert_eq!(accounts, Some(vec![account]));

    let asked = to_the_fetcher(&http);
    assert_eq!(asked.len(), 2, "{asked:?}");
    assert!(over_the_contract(&asked, usenet::CAPABILITY), "{asked:?}");
}

#[tokio::test]
async fn a_usenet_client_speaking_its_contract_says_where_it_files_over_it() {
    let filed = Filed {
        category: "tv".to_owned(),
        folder: "/data/downloads/complete/tv".to_owned(),
    };
    let http = Fake::by_path(vec![(
        "download.usenet/v1/filed",
        Answer::reply(200, json(&vec![filed.clone()])),
    )]);
    let (_project, ctx, fillers) = speaking(
        "downloads-contracted-filing",
        usenet::CAPABILITY,
        true,
        http.clone(),
    );

    let mut read = Vec::new();
    for target in fetching(download_targets(&ctx, &fillers).await) {
        read.extend(target.client().filing().filed().await.unwrap_or_default());
    }

    assert_eq!(read, vec![filed]);
    assert!(over_the_contract(
        &to_the_fetcher(&http),
        usenet::CAPABILITY
    ));
}

#[tokio::test]
async fn a_contracted_client_without_its_key_is_asked_nothing() {
    let http = Fake::always(Answer::reply(200, "[]"));
    let (_project, ctx, fillers) = speaking(
        "downloads-contracted-unkeyed",
        torrent::CAPABILITY,
        false,
        http.clone(),
    );

    let declared = declared_downloads(&ctx, &fillers).await;
    assert!(declared
        .iter()
        .any(|client| client.id == FETCHER && client.target.is_none()));
    for target in &download_targets(&ctx, &fillers).await {
        let _ = read_transfers(target).await;
    }
    assert_eq!(to_the_fetcher(&http), Vec::new());
}

#[test]
fn every_client_is_asked_as_the_protocol_it_moves() {
    let adapter = Contracted::new(
        Fake::silent(),
        "http://127.0.0.1:8080",
        FETCHER,
        CONTRACTED_KEY,
    );
    for moves in [Protocol::Torrent, Protocol::Usenet] {
        let target = DownloadTarget {
            service: FETCHER.to_owned(),
            protocol: moves,
            adapter: adapter.clone(),
        };
        let asked = match target.client() {
            Downloading::Torrent(_) => Protocol::Torrent,
            Downloading::Usenet(_) => Protocol::Usenet,
        };
        assert_eq!(asked, moves);
        assert_eq!(target.torrent().is_some(), moves == Protocol::Torrent);
        assert_eq!(target.usenet().is_some(), moves == Protocol::Usenet);
    }
}
