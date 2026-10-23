use std::path::{Path, PathBuf};
use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_sidecar::gate::{Credential, File, Kind, Upstream, Upstreams, PORT};

use super::{held, rotate, value, SETTING};
use crate::app::Ctx;
use crate::config::{store, Settings};
use crate::credential::{Held, Reach, Settled, State};
use crate::test_support::media::serve;
use crate::test_support::{a_context, json};

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
        dashboard: lemonfiber_manifest::Dashboard::default(),
        media_types: Vec::new(),
        provides: Vec::new(),
        claim: Vec::new(),
        secret: Vec::new(),
        depends_on: Vec::new(),
        grants: Vec::new(),
        host_managed: false,
        memory_mib: None,
        mem_limit_mib: None,
        publish: Vec::new(),
        takes_data: false,
        config_path: None,
        config_ro: false,
        mount: Vec::new(),
        tmpfs: Vec::new(),
        env: std::collections::BTreeMap::new(),
        user_mode: None,
        posture: lemonfiber_manifest::container::Posture::Standard,
        network: None,
        shape: None,
        asks_for: None,
        reaches: None,
        listens: None,
    }
}

/// The media server, and the request gate where `gating`.
fn stack(gating: bool) -> Vec<lemonfiber_manifest::Service> {
    let mut media_server = service(
        "jellyfin",
        Some(lemonfiber_manifest::ApiKind::Jellyfin),
        8096,
    );
    media_server.provides = vec!["identity.source".to_owned()];
    media_server.listens = Some(8096);
    let mut request_service = service("seerr", Some(lemonfiber_manifest::ApiKind::Seerr), 5055);
    request_service.listens = Some(5055);
    let mut services = vec![media_server, request_service];
    if gating {
        services.push(service("request-gate", None, PORT));
    }
    services
}

/// Who fills each of the shipped stack's asks among `services`, written at `project`, with
/// the media server moved into its plugin.
fn filling(services: Vec<lemonfiber_manifest::Service>, project: &Path) -> crate::wiring::Fillers {
    crate::test_support::stack_fillers(
        services,
        &crate::test_support::media::moved()
            .into_iter()
            .collect::<Vec<_>>(),
        Some(project),
        crate::plugin::first_party::EMBEDDED,
    )
}

/// Who fills each of the shipped stack's asks among `services`, written at `project`, with
/// no media server moved in.
fn unserved(services: Vec<lemonfiber_manifest::Service>, project: &Path) -> crate::wiring::Fillers {
    crate::test_support::stack_fillers(
        services,
        &[],
        Some(project),
        crate::plugin::first_party::EMBEDDED,
    )
}

/// The gate's routes: the curator's, and the media server's presenting `key`.
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

/// A stack directory whose gate presents `key` to the media server where it holds routes,
/// and a context whose media server's adapter is keyed there, so it can be asked as the
/// server's administrator, where `administered`, answering over `http`.
fn scene(name: &str, administered: bool, key: Option<&str>, http: Arc<Fake>) -> (Ctx, PathBuf) {
    let at = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(&at);
    let env = at.join(".env");
    let _ = std::fs::write(&env, "DATA_ROOT=/srv/media\n");
    if administered {
        let keyed = crate::plugin::key_file(&at, crate::test_support::media::ADAPTER);
        let _ = std::fs::create_dir_all(keyed.parent().unwrap_or(&at));
        let _ = std::fs::write(&keyed, crate::test_support::media::KEY);
    }
    if let Some(key) = key {
        let _ = store::write(&routes_file(&at), &routes(key).written());
    }
    let ctx = a_context()
        .engine(Arc::new(crate::test_support::media::running()))
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

/// The media server: it answers a mint with `minted`'s status and key, lists each of
/// `filed` in turn as the keys filed under the gate's name, and answers a revocation with
/// `revoked` and a key's proof with `proved`.
fn serving(minted: (u16, &str), filed: &[&[&str]], revoked: u16, proved: u16) -> Arc<Fake> {
    Fake::by_path_in_turn(vec![
        (
            serve("mint"),
            vec![Answer::reply(minted.0, json(&minted.1))],
        ),
        (
            serve("filed_as"),
            filed
                .iter()
                .map(|keys| Answer::reply(200, json(&keys)))
                .collect(),
        ),
        (serve("revoke"), vec![Answer::reply(revoked, "")]),
        (serve("answers_to"), vec![Answer::reply(proved, "")]),
    ])
}

/// The keys a run revoked, in order.
fn revoked(http: &Fake) -> Vec<String> {
    http.requests()
        .into_iter()
        .filter(|asked| asked.url.ends_with(serve("revoke")))
        .filter_map(|asked| serde_json::from_str::<serde_json::Value>(asked.body.as_deref()?).ok())
        .filter_map(|asked| asked.get("key")?.as_str().map(str::to_owned))
        .collect()
}

/// The line, or an empty one the assertions after it then fail on.
async fn listed(ctx: &Ctx, project: &Path) -> Held {
    held(
        ctx,
        &stack(true),
        &filling(stack(true), project),
        Some(project),
    )
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
        serving((200, "fresh"), &[], 204, 204),
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
    let (ctx, at) = scene(
        "gate-held-absent",
        true,
        None,
        serving((200, "fresh"), &[], 204, 204),
    );

    let listed = listed(&ctx, &at).await;

    assert_eq!(listed.state, State::Absent);
    assert!(listed
        .advisory
        .is_some_and(|said| said.contains("lemonfiber seed")));
}

#[tokio::test]
async fn without_the_gate_the_media_server_or_a_stack_directory_there_is_no_line() {
    let (ctx, at) = scene(
        "gate-held-none",
        true,
        Some("kept"),
        serving((200, "fresh"), &[], 204, 204),
    );

    assert!(
        held(&ctx, &stack(false), &filling(stack(false), &at), Some(&at))
            .await
            .is_none()
    );
    assert!(held(&ctx, &stack(true), &filling(stack(true), &at), None)
        .await
        .is_none());
    let gate_alone = vec![service("request-gate", None, PORT)];
    assert!(held(
        &ctx,
        &gate_alone,
        &unserved(gate_alone.clone(), &at),
        Some(&at)
    )
    .await
    .is_none());
    let contracted = crate::test_support::stack_fillers(
        gate_alone.clone(),
        &[crate::test_support::a_contracted_media_server(Some(
            "upstream",
        ))],
        None,
        crate::plugin::first_party::EMBEDDED,
    );
    assert!(crate::app::targets::MediaServer::of(&contracted).is_some());
    assert!(held(&ctx, &gate_alone, &contracted, Some(&at))
        .await
        .is_none());
    let rotation = rotate(&ctx, &routes_held(&at), &contracted).await;
    assert!(unproven(&rotation.settled)
        .is_some_and(|said| said == crate::app::credentials::declining::NO_ADMINISTRATOR));
}

#[tokio::test]
async fn a_confirmed_ask_prints_the_key_from_the_routes() {
    let (ctx, at) = scene(
        "gate-reveal",
        true,
        Some("kept"),
        serving((200, "fresh"), &[], 204, 204),
    );

    let listed = listed(&ctx, &at).await;
    let shown = super::super::revealing::reveal(&ctx, &listed, true).await;

    assert_eq!(shown.value.as_deref(), Some("kept"));
    assert_eq!(value(&ctx, &listed).await.as_deref(), Some("kept"));
}

#[tokio::test]
async fn a_rotation_lands_only_once_the_media_server_takes_the_key() {
    let http = serving((200, "fresh"), &[&["old", "fresh"]], 204, 204);
    let (ctx, at) = scene("gate-rotate-landed", true, Some("old"), http.clone());
    let listed = listed(&ctx, &at).await;

    let rotation = super::super::rotating::rotate(
        &ctx,
        &listed,
        &stack(true),
        &filling(stack(true), &at),
        Some(&at),
    )
    .await;

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
async fn a_new_key_the_media_server_refuses_is_revoked_and_the_old_routes_put_back() {
    let http = serving((200, "fresh"), &[&["old", "fresh"]], 204, 401);
    let (ctx, at) = scene("gate-rotate-untaken", true, Some("old"), http.clone());
    let listed = listed(&ctx, &at).await;

    let rotation = rotate(&ctx, &listed, &filling(stack(true), &at)).await;

    assert!(unproven(&rotation.settled)
        .is_some_and(|said| said.starts_with("The media server did not take")));
    assert_eq!(on_disk(&at), Some(routes("old")));
    assert_eq!(revoked(&http), vec!["fresh".to_owned()]);
}

#[tokio::test]
async fn a_new_key_that_cannot_be_written_is_revoked_again() {
    for (name, minted, blocked) in [
        ("gate-rotate-unwritten", "fresh", true),
        ("gate-rotate-two-words", "two words", false),
    ] {
        let http = serving((200, minted), &[&["old", minted]], 204, 204);
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
            let rotation = rotate(&ctx, &listed, &filling(stack(true), &at)).await;
            assert!(
                unproven(&rotation.settled)
                    .is_some_and(|said| said.starts_with("the new key could not")),
                "{name}: {rotation:?}"
            );
        } else {
            let rotation = rotate(&ctx, &listed, &filling(stack(true), &at)).await;
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
async fn a_mint_the_media_server_refuses_or_routes_not_there_change_nothing() {
    let http = serving((500, ""), &[&["old"]], 204, 204);
    let (ctx, at) = scene("gate-rotate-unminted", true, Some("old"), http.clone());
    let listed = listed(&ctx, &at).await;
    let rotation = rotate(&ctx, &listed, &filling(stack(true), &at)).await;
    assert!(unproven(&rotation.settled).is_some(), "{rotation:?}");
    assert_eq!(on_disk(&at), Some(routes("old")));
    assert!(revoked(&http).is_empty());

    let http = serving((200, "fresh"), &[], 204, 204);
    let (ctx, at) = scene("gate-rotate-unrouted", true, None, http.clone());
    let unrouted = held(&ctx, &stack(true), &filling(stack(true), &at), Some(&at)).await;
    let rotation = match unrouted {
        Some(line) => rotate(&ctx, &line, &filling(stack(true), &at)).await,
        None => rotate(&ctx, &routes_held(&at), &filling(stack(true), &at)).await,
    };
    assert!(
        unproven(&rotation.settled).is_some_and(|said| said.contains("lemonfiber seed")),
        "{rotation:?}"
    );
    assert!(!http
        .requests()
        .iter()
        .any(|asked| asked.url.ends_with(serve("mint"))));
}

#[tokio::test]
async fn without_an_administrator_or_on_a_rehearsal_nothing_is_minted() {
    let http = serving((200, "fresh"), &[&["old"]], 204, 204);
    let (ctx, at) = scene(
        "gate-rotate-unadministered",
        false,
        Some("old"),
        http.clone(),
    );
    let listed = listed(&ctx, &at).await;

    let unadministered = rotate(&ctx, &listed, &filling(stack(true), &at)).await;
    assert!(unproven(&unadministered.settled).is_some_and(|said| said.contains("no administrator")));
    let gate_alone = vec![service("request-gate", None, PORT)];
    let serverless = rotate(&ctx, &listed, &unserved(gate_alone, &at)).await;
    assert!(unproven(&serverless.settled)
        .is_some_and(|said| said == crate::app::credentials::declining::NO_ADMINISTRATOR));

    let (mut rehearsing, keyed) = scene("gate-rotate-rehearsed", true, Some("old"), http.clone());
    rehearsing.dry_run = true;
    let rehearsed = rotate(&rehearsing, &listed, &filling(stack(true), &keyed)).await;
    assert!(
        matches!(rehearsed.settled, Settled::Rehearsed { .. }),
        "{rehearsed:?}"
    );

    assert!(http.requests().is_empty());
    assert_eq!(on_disk(&at), Some(routes("old")));
}
