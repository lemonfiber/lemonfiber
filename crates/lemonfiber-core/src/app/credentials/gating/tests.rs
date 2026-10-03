use std::path::{Path, PathBuf};
use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_sidecar::gate::{Credential, File, Kind, Upstream, Upstreams, PORT};

use super::{held, rotate, value, SETTING};
use crate::app::Ctx;
use crate::config::{store, Settings};
use crate::credential::{Held, Reach, Settled, State};
use crate::test_support::a_context;

/// A service in the stack, with the shape this module reads.
fn service(
    id: &str,
    api: Option<lemonfiber_manifest::ApiKind>,
    port: u16,
) -> lemonfiber_manifest::Service {
    lemonfiber_manifest::Service {
        id: id.to_owned(),
        name: format!("{id} the app"),
        profile: "media".to_owned(),
        image: "example/image".to_owned(),
        tag: "1".to_owned(),
        digest: None,
        port: Some(port),
        bind: None,
        health: None,
        api: api.map(|kind| lemonfiber_manifest::Api {
            kind,
            key_source: lemonfiber_manifest::KeySource::Generated,
            path: None,
            version: None,
        }),
        criticality: lemonfiber_manifest::Criticality::Core,
        license: "MIT".to_owned(),
        upstream: "https://example.test".to_owned(),
        last_release: "2026-01-01".to_owned(),
        describes: "an example service".to_owned(),
        without_it: "nothing works".to_owned(),
        media_types: Vec::new(),
        provides: Vec::new(),
        claim: Vec::new(),
        depends_on: Vec::new(),
        grants: Vec::new(),
        host_managed: false,
        memory_mib: None,
        asks_for: None,
        reaches: None,
    }
}

/// Jellyfin, and the request gate where `gating`.
fn stack(gating: bool) -> Vec<lemonfiber_manifest::Service> {
    let mut services = vec![service(
        "jellyfin",
        Some(lemonfiber_manifest::ApiKind::Jellyfin),
        8096,
    )];
    if gating {
        services.push(service("request-gate", None, PORT));
    }
    services
}

/// The gate's routes: Sonarr's, and Jellyfin's presenting `key`.
fn routes(key: &str) -> Upstreams {
    Upstreams::of(vec![
        Upstream {
            route: "sonarr".to_owned(),
            kind: Kind::Sonarr,
            address: "http://sonarr:8989".to_owned(),
            credential: Credential::new("sonarrs"),
            majors: Vec::new(),
        },
        Upstream {
            route: "jellyfin".to_owned(),
            kind: Kind::Jellyfin,
            address: "http://jellyfin:8096".to_owned(),
            credential: Credential::new(key),
            majors: vec![10],
        },
    ])
}

/// A stack directory whose gate presents `key` to Jellyfin where it holds routes, and
/// a context with the media server's administrator recorded where `administered`,
/// answering over `http`.
fn scene(name: &str, administered: bool, key: Option<&str>, http: Arc<Fake>) -> (Ctx, PathBuf) {
    let at = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(&at);
    let env = at.join(".env");
    let _ = std::fs::write(&env, "DATA_ROOT=/srv/media\n");
    if administered {
        let _ = store::set(
            &env,
            crate::config::JELLYFIN_ADMIN_PASSWORD_KEY,
            &lemonfiber_fixtures::support::a_password(),
        );
    }
    if let Some(key) = key {
        let _ = store::write(&routes_file(&at), &routes(key).written());
    }
    let ctx = a_context()
        .settings(Settings {
            env_file: Some(env),
            ..Settings::default()
        })
        .build()
        .with_http(http);
    (ctx, at)
}

fn routes_file(project: &Path) -> PathBuf {
    crate::app::gating::path(project, File::Upstreams)
}

/// What the routes under `project` hold, where they read.
fn on_disk(project: &Path) -> Option<Upstreams> {
    std::fs::read_to_string(routes_file(project))
        .ok()
        .and_then(|text| Upstreams::read(&text).ok())
}

/// Jellyfin: it lists each of `lists` in turn under the gate's name, and answers a mint
/// with `minted`, a revocation with `revoked` and a key's proof with `proved`.
fn serving(lists: &[&[&str]], minted: u16, revoked: u16, proved: u16) -> Arc<Fake> {
    let listed = |keys: &[&str]| {
        let items: Vec<serde_json::Value> = keys
            .iter()
            .map(|key| serde_json::json!({ "AppName": crate::jellyfin::GATE_APP, "AccessToken": key }))
            .collect();
        Answer::reply(200, serde_json::json!({ "Items": items }).to_string())
    };
    Fake::by_route_in_turn(vec![
        (
            Method::Post,
            "/Users/AuthenticateByName",
            vec![Answer::reply(200, r#"{"AccessToken":"token"}"#)],
        ),
        (
            Method::Get,
            "/Auth/Keys",
            lists.iter().map(|keys| listed(keys)).collect(),
        ),
        (Method::Post, "/Auth/Keys", vec![Answer::reply(minted, "")]),
        (
            Method::Delete,
            "/Auth/Keys/",
            vec![Answer::reply(revoked, "")],
        ),
        (
            Method::Get,
            "/System/Info",
            vec![Answer::reply(proved, "{}")],
        ),
    ])
}

/// The keys a run revoked, in order.
fn revoked(http: &Fake) -> Vec<String> {
    http.requests()
        .into_iter()
        .filter(|asked| asked.method == Method::Delete)
        .filter_map(|asked| asked.url.rsplit('/').next().map(str::to_owned))
        .collect()
}

/// The line, or an empty one the assertions after it then fail on.
async fn listed(ctx: &Ctx, project: &Path) -> Held {
    held(ctx, &stack(true), Some(project))
        .await
        .unwrap_or_else(|| Held {
            name: String::new(),
            setting: String::new(),
            consumers: Vec::new(),
            location: String::new(),
            origin: crate::credential::Origin::Lemonfiber,
            from: crate::origin::Origin::Bundled,
            state: State::Absent,
            fingerprint: None,
            advisory: None,
        })
}

/// A line naming the routes under `project`, for the case the inventory lists none.
fn routes_held(project: &Path) -> Held {
    Held {
        name: "Jellyfin request-gate key".to_owned(),
        setting: SETTING.to_owned(),
        consumers: Vec::new(),
        location: routes_file(project).display().to_string(),
        origin: crate::credential::Origin::Lemonfiber,
        from: crate::origin::Origin::Bundled,
        state: State::Absent,
        fingerprint: None,
        advisory: None,
    }
}

fn unproven(settled: &Settled) -> Option<&str> {
    match settled {
        Settled::Unproven { detail } => Some(detail),
        _ => None,
    }
}

#[tokio::test]
async fn the_key_is_listed_where_the_stack_runs_the_gate() {
    let (ctx, at) = scene(
        "gate-held-present",
        true,
        Some("kept"),
        serving(&[], 204, 204, 200),
    );

    let listed = listed(&ctx, &at).await;

    assert_eq!(listed.state, State::Active);
    assert_eq!(listed.setting, SETTING);
    assert_eq!(listed.name, "Jellyfin request-gate key");
    assert_eq!(
        listed.fingerprint,
        Some(crate::credential::fingerprint("kept"))
    );
    assert_eq!(listed.location, routes_file(&at).display().to_string());
    assert!(listed.advisory.is_none());
}

#[tokio::test]
async fn a_key_not_there_yet_is_absent_and_says_how_to_get_one() {
    let (ctx, at) = scene("gate-held-absent", true, None, serving(&[], 204, 204, 200));

    let listed = listed(&ctx, &at).await;

    assert_eq!(listed.state, State::Absent);
    assert!(listed
        .advisory
        .is_some_and(|said| said.contains("lemonfiber seed")));
}

#[tokio::test]
async fn without_the_gate_jellyfin_or_a_stack_directory_there_is_no_line() {
    let (ctx, at) = scene(
        "gate-held-none",
        true,
        Some("kept"),
        serving(&[], 204, 204, 200),
    );

    assert!(held(&ctx, &stack(false), Some(&at)).await.is_none());
    assert!(held(&ctx, &stack(true), None).await.is_none());
    let gate_alone = vec![service("request-gate", None, PORT)];
    assert!(held(&ctx, &gate_alone, Some(&at)).await.is_none());
}

#[tokio::test]
async fn a_confirmed_ask_prints_the_key_from_the_routes() {
    let (ctx, at) = scene(
        "gate-reveal",
        true,
        Some("kept"),
        serving(&[], 204, 204, 200),
    );

    let listed = listed(&ctx, &at).await;
    let shown = super::super::revealing::reveal(&ctx, &listed, true).await;

    assert_eq!(shown.value.as_deref(), Some("kept"));
    assert_eq!(value(&ctx, &listed).await.as_deref(), Some("kept"));
}

#[tokio::test]
async fn a_rotation_lands_only_once_jellyfin_takes_the_key() {
    let http = serving(
        &[&["old"], &["old", "fresh"], &["old", "fresh"]],
        204,
        204,
        200,
    );
    let (ctx, at) = scene("gate-rotate-landed", true, Some("old"), http.clone());
    let listed = listed(&ctx, &at).await;

    let rotation = super::super::rotating::rotate(&ctx, &listed, &stack(true), Some(&at)).await;

    assert!(
        matches!(rotation.settled, Settled::Replaced { .. }),
        "{rotation:?}"
    );
    assert_eq!(on_disk(&at), Some(routes("fresh")));
    assert_eq!(revoked(&http), vec!["old".to_owned()]);
    assert_eq!(
        rotation.consumers.first().map(|one| one.reach.clone()),
        Some(Reach::Updated)
    );
}

#[tokio::test]
async fn a_new_key_jellyfin_refuses_is_revoked_and_the_old_routes_put_back() {
    let http = serving(&[&["old"], &["old", "fresh"]], 204, 204, 401);
    let (ctx, at) = scene("gate-rotate-untaken", true, Some("old"), http.clone());
    let listed = listed(&ctx, &at).await;

    let rotation = rotate(&ctx, &listed, &stack(true)).await;

    assert!(
        unproven(&rotation.settled).is_some_and(|said| said.starts_with("Jellyfin did not take"))
    );
    assert_eq!(on_disk(&at), Some(routes("old")));
    assert_eq!(revoked(&http), vec!["fresh".to_owned()]);
}

#[tokio::test]
async fn a_new_key_that_cannot_be_written_is_revoked_again() {
    for (name, minted, blocked) in [
        ("gate-rotate-unwritten", "fresh", true),
        ("gate-rotate-two-words", "two words", false),
    ] {
        let http = serving(&[&["old"], &["old", minted]], 204, 204, 200);
        let (ctx, at) = scene(name, true, Some("old"), http.clone());
        let listed = listed(&ctx, &at).await;
        if blocked {
            // Read through a filesystem that still shows the routes, while the real
            // directory leaves nowhere to write them.
            let held_text = routes("old").written();
            let _ = std::fs::remove_file(routes_file(&at));
            let _ = std::fs::create_dir_all(routes_file(&at).join("blocked"));
            let ctx = ctx.with_filesystem(lemonfiber_fixtures::files::Files::at(vec![(
                routes_file(&at),
                &held_text,
            )]));
            let rotation = rotate(&ctx, &listed, &stack(true)).await;
            assert!(
                unproven(&rotation.settled)
                    .is_some_and(|said| said.starts_with("the new key could not")),
                "{name}: {rotation:?}"
            );
        } else {
            let rotation = rotate(&ctx, &listed, &stack(true)).await;
            assert!(
                unproven(&rotation.settled)
                    .is_some_and(|said| said.starts_with("the new key could not")),
                "{name}: {rotation:?}"
            );
            assert_eq!(on_disk(&at), Some(routes("old")), "{name}");
        }
        assert_eq!(revoked(&http), vec![minted.to_owned()], "{name}");
    }
}

#[tokio::test]
async fn a_mint_jellyfin_refuses_or_routes_not_there_change_nothing() {
    let http = serving(&[&["old"]], 500, 204, 200);
    let (ctx, at) = scene("gate-rotate-unminted", true, Some("old"), http.clone());
    let listed = listed(&ctx, &at).await;
    let rotation = rotate(&ctx, &listed, &stack(true)).await;
    assert!(unproven(&rotation.settled).is_some(), "{rotation:?}");
    assert_eq!(on_disk(&at), Some(routes("old")));
    assert!(revoked(&http).is_empty());

    let http = serving(&[&[]], 204, 204, 200);
    let (ctx, at) = scene("gate-rotate-unrouted", true, None, http.clone());
    let unrouted = held(&ctx, &stack(true), Some(&at)).await;
    let rotation = match unrouted {
        Some(line) => rotate(&ctx, &line, &stack(true)).await,
        None => rotate(&ctx, &routes_held(&at), &stack(true)).await,
    };
    assert!(
        unproven(&rotation.settled).is_some_and(|said| said.contains("lemonfiber seed")),
        "{rotation:?}"
    );
    assert!(!http
        .requests()
        .iter()
        .any(|asked| asked.method == Method::Post && asked.url.contains("/Auth/Keys")));
}

#[tokio::test]
async fn without_an_administrator_or_on_a_rehearsal_nothing_is_minted() {
    let http = serving(&[&["old"]], 204, 204, 200);
    let (ctx, at) = scene(
        "gate-rotate-unadministered",
        false,
        Some("old"),
        http.clone(),
    );
    let listed = listed(&ctx, &at).await;

    let unadministered = rotate(&ctx, &listed, &stack(true)).await;
    assert!(unproven(&unadministered.settled).is_some_and(|said| said.contains("no administrator")));

    let (mut rehearsing, _) = scene("gate-rotate-rehearsed", true, Some("old"), http.clone());
    rehearsing.dry_run = true;
    let rehearsed = rotate(&rehearsing, &listed, &stack(true)).await;
    assert!(
        matches!(rehearsed.settled, Settled::Rehearsed { .. }),
        "{rehearsed:?}"
    );

    assert!(http.requests().is_empty());
    assert_eq!(on_disk(&at), Some(routes("old")));
}
