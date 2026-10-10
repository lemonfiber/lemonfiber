use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};

use super::{
    declared_downloads, download_targets, forwarded_client, read_transfers, torrent_client,
    DownloadKind, DownloadTarget,
};
use crate::config::Settings;
use crate::test_support::{a_context, a_password, a_placed, an_installed, env_at, stack};
use crate::wiring::{Chosen, Fillers};

/// The port the plugin's torrent client publishes on the host.
const BROUGHT_PORT: u16 = 9091;

/// The password recorded for the plugin's torrent client alone.
const BROUGHT_PASSWORD: &str = "the-plugins-own";

/// The embedded stack beside a plugin's torrent client, reached on its own loopback port.
fn beside_a_plugin_client() -> Fillers {
    let api = lemonfiber_manifest::Api {
        kind: lemonfiber_manifest::ApiKind::Qbittorrent,
        key_source: lemonfiber_manifest::KeySource::ConfigIni,
        path: None,
        version: None,
    };
    let client = a_placed(
        "seedbox",
        &["download.torrent"],
        Some(api),
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
        .filter(|target| {
            matches!(&target.kind, DownloadKind::Torrent { base, .. } if base.ends_with(&format!(":{BROUGHT_PORT}")))
        })
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
    let api = lemonfiber_manifest::Api {
        kind: lemonfiber_manifest::ApiKind::Qbittorrent,
        key_source: lemonfiber_manifest::KeySource::ConfigIni,
        path: None,
        version: None,
    };
    let unpublished = a_placed("portless", &["download.torrent"], Some(api), None);
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
