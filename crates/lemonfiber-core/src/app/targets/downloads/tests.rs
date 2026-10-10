use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};

use lemonfiber_contract::capabilities::download::{torrent, usenet};
use lemonfiber_contract::Contracted;
use lemonfiber_fixtures::scratch::Scratch;

use super::{
    declared_downloads, download_targets, forwarded_client, protocol_of, read_transfers,
    torrent_client, DownloadKind, DownloadTarget, Downloading,
};
use crate::config::Settings;
use crate::dashboard::Protocol;
use crate::ports::service::{Download, Seeded, UsenetAccount};
use crate::test_support::{
    a_context, a_password, a_placed, an_installed, contracted, contracted_context, env_at,
    first_party, json, stack, CONTRACTED_KEY,
};
use crate::wiring::{Chosen, Fillers};

/// The port the plugin's torrent client publishes on the host.
const BROUGHT_PORT: u16 = 9091;

/// The password recorded for the plugin's torrent client alone.
const BROUGHT_PASSWORD: &str = "the-plugins-own";

/// The bundled torrent client's adapter, as a plugin's service names it.
const fn torrent_api() -> lemonfiber_manifest::Api {
    lemonfiber_manifest::Api {
        kind: lemonfiber_manifest::ApiKind::Qbittorrent,
        key_source: lemonfiber_manifest::KeySource::ConfigIni,
        path: None,
        version: None,
    }
}

/// The embedded stack beside a plugin's torrent client, reached on its own loopback port.
fn beside_a_plugin_client() -> Fillers {
    let client = a_placed(
        "seedbox",
        &["download.torrent"],
        Some(torrent_api()),
        Some(BROUGHT_PORT),
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

/// A context holding the stack's torrent client's password, and the plugin's own where
/// `brought` says so, over a transport answering every torrent client.
fn holding(name: &str, brought: bool, fillers: &Fillers) -> (crate::app::Ctx, Arc<Fake>) {
    let env = env_at(name, &a_password());
    if brought {
        let setting = fillers
            .service("seedbox")
            .and_then(|filler| fillers.setting(filler, crate::config::PASSWORD_SUFFIX))
            .unwrap_or_default();
        assert!(crate::config::store::set(&env, &setting, BROUGHT_PASSWORD).is_ok());
    }
    let http = Fake::by_path(vec![
        ("auth/login", Answer::reply(200, "Ok.")),
        ("torrents/info", Answer::reply(200, "[]")),
        (
            "app/preferences",
            Answer::reply(200, r#"{"listen_port":51413}"#),
        ),
    ]);
    let ctx = a_context()
        .settings(Settings {
            env_file: Some(env),
            ..Settings::default()
        })
        .build()
        .with_http(http.clone());
    (ctx, http)
}

/// Each torrent client's password, by the port it is reached on.
fn passwords(targets: &[super::DownloadTarget]) -> Vec<(String, String)> {
    targets
        .iter()
        .filter_map(|target| match &target.kind {
            DownloadKind::Torrent { base, password } => Some((base.clone(), password.clone())),
            DownloadKind::Usenet { .. } | DownloadKind::Over { .. } => None,
        })
        .collect()
}

/// A plugin's torrent client is never sent the stack's password: with none recorded
/// for it, it is not read at all, and with its own recorded it is read with that one.
#[tokio::test]
async fn a_plugin_torrent_client_never_gets_the_stacks_password() {
    let fillers = beside_a_plugin_client();
    let brought = format!("127.0.0.1:{BROUGHT_PORT}");

    let (bare, bare_http) = holding("plugin-client-bare", false, &fillers);
    for target in &download_targets(&bare, &fillers).await {
        let _ = read_transfers(&bare, target).await;
    }
    let (kept, kept_http) = holding("plugin-client-kept", true, &fillers);
    let targets = download_targets(&kept, &fillers).await;
    for target in &targets {
        let _ = read_transfers(&kept, target).await;
    }

    assert!(
        bare_http
            .requests()
            .iter()
            .all(|asked| !asked.url.contains(&brought)),
        "{:?}",
        bare_http.requests()
    );
    assert!(
        passwords(&targets).contains(&(format!("http://{brought}"), BROUGHT_PASSWORD.to_owned()))
    );
    let to_the_plugin: Vec<_> = kept_http
        .requests()
        .into_iter()
        .filter(|asked| asked.url.contains(&brought))
        .collect();
    assert!(!to_the_plugin.is_empty());
    assert!(
        to_the_plugin.iter().all(|asked| {
            !asked
                .body
                .as_deref()
                .unwrap_or_default()
                .contains(&a_password())
        }),
        "{to_the_plugin:?}"
    );
}

/// The forwarded port belongs to the torrent client reached through the tunnel, so a
/// plugin's client, which never is, is not the one read or moved for it — even where it
/// is the only torrent client lemonfiber can authenticate to.
#[tokio::test]
async fn a_plugin_torrent_client_is_never_the_forwarded_one() {
    let fillers = beside_a_plugin_client();
    let (ctx, http) = holding("plugin-client-forwarded", true, &fillers);
    let targets = download_targets(&ctx, &fillers).await;
    let brought: Vec<_> = targets
        .into_iter()
        .filter(|target| target.service == "seedbox")
        .collect();

    assert!(brought.iter().all(|target| !target.tunnelled));
    assert!(forwarded_client(&ctx, &brought).is_none());
    assert!(torrent_client(&ctx, &brought).is_some());
    assert!(http.requests().is_empty());
}

/// The stack's own torrent client goes through the tunnel and is read with the password
/// recorded for it.
#[tokio::test]
async fn the_stacks_torrent_client_is_the_forwarded_one_and_holds_its_own_password() {
    let fillers = beside_a_plugin_client();
    let (ctx, _) = holding("stack-client-forwarded", false, &fillers);
    let targets = download_targets(&ctx, &fillers).await;

    assert_eq!(
        passwords(&targets),
        vec![("http://127.0.0.1:8081".to_owned(), a_password())]
    );
    assert!(targets.iter().all(|target| target.tunnelled));
    assert!(forwarded_client(&ctx, &targets).is_some());
}

/// A record of what is installed that will not read narrows a host read to the stack's
/// own services rather than failing it.
#[test]
fn an_unreadable_register_leaves_the_stacks_own_services() {
    let env = env_at("host-unread", &a_password());
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
    let ctx = a_context().build();
    let targets = [
        DownloadTarget {
            service: "usenet".to_owned(),
            kind: DownloadKind::Usenet {
                base: "http://127.0.0.1:8085".to_owned(),
                key: "usenet-key".to_owned(),
            },
            tunnelled: true,
        },
        DownloadTarget {
            service: "torrent".to_owned(),
            kind: DownloadKind::Torrent {
                base: "http://127.0.0.1:8081".to_owned(),
                password: a_password(),
            },
            tunnelled: true,
        },
    ];

    assert!(torrent_client(&ctx, &targets[..1]).is_none());
    assert!(forwarded_client(&ctx, &targets[..1]).is_none());
    assert!(torrent_client(&ctx, &targets).is_some());
}

/// Every download client is declared, the ones nothing here can reach included: a
/// torrent client publishing no port, and one whose password is not recorded, are
/// named with nothing to reach them by, where the read leaves both out.
#[tokio::test]
async fn every_download_client_is_declared_whether_or_not_it_is_reached() {
    let unpublished = a_placed("portless", &["download.torrent"], Some(torrent_api()), None);
    let fillers = stack()
        .manifest()
        .map(|manifest| {
            Fillers::of(
                &manifest,
                &[an_installed("seeding", vec![unpublished])],
                &Chosen::default(),
                None,
            )
        })
        .unwrap_or_default();
    let (ctx, _) = holding("declared-unpublished", false, &fillers);
    let declared = declared_downloads(&ctx, &fillers).await;
    assert!(declared
        .iter()
        .any(|client| client.id == "portless" && client.target.is_none()));
    assert!(declared
        .iter()
        .any(|client| client.id == "qbittorrent" && client.target.is_some()));
    assert_eq!(
        download_targets(&ctx, &fillers).await.len(),
        declared
            .iter()
            .filter(|client| client.target.is_some())
            .count()
    );
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
/// beside the stack's own clients. It names the bundled torrent client's adapter too,
/// with a password recorded for it, so asking it the bundled way is open to a read
/// that falls back.
fn speaking(
    name: &str,
    capability: &str,
    keyed: bool,
    http: Arc<Fake>,
) -> (Scratch, crate::app::Ctx, Fillers) {
    let project = Scratch::new(name);
    let mut installed = contracted("downloading", FETCHER, capability);
    for placed in &mut installed.services {
        placed.api = Some(torrent_api());
    }
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
    let env = env_at(name, &a_password());
    let setting = fillers
        .service(FETCHER)
        .and_then(|filler| fillers.setting(filler, crate::config::PASSWORD_SUFFIX))
        .unwrap_or_default();
    assert!(crate::config::store::set(&env, &setting, BROUGHT_PASSWORD).is_ok());
    let ctx = contracted_context(&project, FETCHER, keyed)
        .settings(Settings {
            env_file: Some(env),
            ..Settings::default()
        })
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
            [one] if matches!(one.kind, DownloadKind::Over { protocol: Protocol::Torrent, .. })
                && !one.tunnelled
        ),
        "asked over the contract, never as the bundled client with the password recorded"
    );
    let mut read = Vec::new();
    for target in &targets {
        read.extend(read_transfers(&ctx, target).await);
    }
    assert_eq!(read, vec![arriving()]);
    let seeding = match torrent_client(&ctx, &targets) {
        Some(holder) => holder.seeding().await.ok(),
        None => None,
    };
    assert_eq!(seeding, Some(vec![held]));
    assert!(forwarded_client(&ctx, &targets).is_none());

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
        [one] if protocol_of(&one.kind) == Protocol::Usenet
    ));
    let mut read = Vec::new();
    for target in &targets {
        read.extend(read_transfers(&ctx, target).await);
    }
    assert_eq!(read, vec![arriving()]);
    assert!(torrent_client(&ctx, &targets).is_none());
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
        let _ = read_transfers(&ctx, target).await;
    }
    assert_eq!(to_the_fetcher(&http), Vec::new());
}

#[test]
fn every_kind_of_client_is_asked_as_the_protocol_it_moves() {
    let ctx = a_context().build();
    let adapter = Contracted::new(
        Fake::silent(),
        "http://127.0.0.1:8080",
        FETCHER,
        CONTRACTED_KEY,
    );
    let kinds = [
        (
            DownloadKind::Torrent {
                base: "http://127.0.0.1:8081".to_owned(),
                password: a_password(),
            },
            Protocol::Torrent,
        ),
        (
            DownloadKind::Usenet {
                base: "http://127.0.0.1:8085".to_owned(),
                key: "usenet-key".to_owned(),
            },
            Protocol::Usenet,
        ),
        (
            DownloadKind::Over {
                protocol: Protocol::Torrent,
                adapter: adapter.clone(),
            },
            Protocol::Torrent,
        ),
        (
            DownloadKind::Over {
                protocol: Protocol::Usenet,
                adapter,
            },
            Protocol::Usenet,
        ),
    ];
    let mut forwarded = Vec::new();
    for (kind, moves) in kinds {
        let target = DownloadTarget {
            service: FETCHER.to_owned(),
            kind,
            tunnelled: true,
        };
        let asked = match target.client(&ctx) {
            Downloading::Torrent(_) => Protocol::Torrent,
            Downloading::Usenet(_) => Protocol::Usenet,
        };
        assert_eq!((protocol_of(&target.kind), asked), (moves, moves));
        assert_eq!(target.torrent(&ctx).is_some(), moves == Protocol::Torrent);
        assert_eq!(target.usenet(&ctx).is_some(), moves == Protocol::Usenet);
        forwarded.push(forwarded_client(&ctx, std::slice::from_ref(&target)).is_some());
    }
    assert_eq!(
        forwarded,
        [true, false, false, false],
        "only the bundled torrent client is offered the forwarded port"
    );
}
