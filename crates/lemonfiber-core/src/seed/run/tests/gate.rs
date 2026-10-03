//! The request gate's routes: each upstream it answers for, and the key it holds there.

use lemonfiber_sidecar::gate::{Credential, File, Kind, Upstream, Upstreams};

use super::*;

/// The name the gate's Jellyfin key is filed under.
const APP: &str = crate::jellyfin::GATE_APP;

/// A stack with the media server, the request service, Sonarr, Radarr and Lidarr — which
/// files music the request service never asks for, so the gate has no route to it — and,
/// where `gating`, the request gate.
fn stack(gating: bool) -> Vec<lemonfiber_manifest::Service> {
    let mut services = vec![
        jellyfin_svc(),
        seerr_svc(),
        arr("sonarr", 8989, "tv"),
        arr("radarr", 7878, "movies"),
        arr("lidarr", 8686, "music"),
    ];
    if gating {
        services.push(manifest_service(
            "request-gate",
            None,
            Some(lemonfiber_sidecar::gate::PORT),
        ));
    }
    services
}

/// Jellyfin's key list, holding `keys` under the gate's name and Seerr's own key beside
/// them.
fn listed(keys: &[&str]) -> String {
    let mut items: Vec<serde_json::Value> = keys
        .iter()
        .map(|key| serde_json::json!({ "AppName": APP, "AccessToken": key }))
        .collect();
    items.push(serde_json::json!({ "AppName": "Jellyseerr", "AccessToken": "seerrs" }));
    serde_json::json!({ "Items": items }).to_string()
}

/// A media server answering its key list with each of `lists` in turn, a mint with
/// `minted` and a revocation with `revoked`.
fn serving(lists: &[&[&str]], minted: u16, revoked: u16) -> Arc<Fake> {
    Fake::by_route_in_turn(vec![
        (
            Method::Post,
            "/Users/AuthenticateByName",
            vec![Answer::reply(200, r#"{"AccessToken":"token"}"#)],
        ),
        (
            Method::Get,
            "/Auth/Keys",
            lists
                .iter()
                .map(|keys| Answer::reply(200, listed(keys)))
                .collect(),
        ),
        (Method::Post, "/Auth/Keys", vec![Answer::reply(minted, "")]),
        (
            Method::Delete,
            "/Auth/Keys/",
            vec![Answer::reply(revoked, "")],
        ),
    ])
}

/// A stack directory where each of `arrs` has written its own key, holding `held` as the
/// gate's Jellyfin key where it holds one, and a context with the media server's
/// administrator recorded where `administered` says, answering over `http`.
fn gate_ctx(
    name: &str,
    administered: bool,
    arrs: &[(&str, &str)],
    held: Option<&str>,
    http: Arc<Fake>,
) -> (Ctx, std::path::PathBuf) {
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
    for (arr, key) in arrs {
        let config = at.join("config").join(arr).join("config.xml");
        let _ = store::write(&config, &format!("<Config><ApiKey>{key}</ApiKey></Config>"));
    }
    if let Some(held) = held {
        let _ = store::write(&routes_file(&at), &holding(arrs, held).written());
    }
    let ctx = a_context()
        .environment(crate::platform::Environment::LinuxNative)
        .settings(Settings {
            env_file: Some(env),
            ..Settings::default()
        })
        .build()
        .with_http(http);
    (ctx, at)
}

/// The routes the gate should hold where `arrs` have written their keys and `key` is
/// its Jellyfin key.
fn holding(arrs: &[(&str, &str)], key: &str) -> Upstreams {
    let mut upstreams: Vec<Upstream> = arrs
        .iter()
        .map(|(arr, credential)| Upstream {
            route: (*arr).to_owned(),
            kind: if *arr == "sonarr" {
                Kind::Sonarr
            } else {
                Kind::Radarr
            },
            address: format!(
                "http://{arr}:{}",
                if *arr == "sonarr" { 8989 } else { 7878 }
            ),
            credential: Credential::new(*credential),
            majors: Vec::new(),
        })
        .collect();
    upstreams.push(Upstream {
        route: "jellyfin".to_owned(),
        kind: Kind::Jellyfin,
        address: jellyfin_svc_network_url(),
        credential: Credential::new(key),
        majors: jellyfin_svc().majors(),
    });
    Upstreams::of(upstreams)
}

/// Where the gate reaches Jellyfin, as the seed step resolves it.
fn jellyfin_svc_network_url() -> String {
    let services = [jellyfin_svc()];
    crate::seed::run::identity::jellyfin_service(&services)
        .map(|jellyfin| jellyfin.network_url)
        .unwrap_or_default()
}

/// Where the gate reads its routes, under `project`.
fn routes_file(project: &std::path::Path) -> std::path::PathBuf {
    crate::app::gating::path(project, File::Upstreams)
}

/// What the routes file under `project` holds, where it reads.
fn routes(project: &std::path::Path) -> Option<Upstreams> {
    std::fs::read_to_string(routes_file(project))
        .ok()
        .and_then(|text| Upstreams::read(&text).ok())
}

/// The one wiring the step reports, and the keys it revoked.
async fn seeded(
    ctx: &Ctx,
    http: &Fake,
    gating: bool,
    project: &std::path::Path,
) -> (Option<State>, Vec<String>) {
    let wiring = super::super::gate::seed_gate_routes(ctx, &stack(gating), Some(project)).await;
    let revoked = http
        .requests()
        .into_iter()
        .filter(|asked| asked.method == Method::Delete)
        .filter_map(|asked| asked.url.rsplit('/').next().map(str::to_owned))
        .collect();
    (wiring.map(|one| one.state), revoked)
}

/// Both \*arrs, with the keys they wrote.
const ARRS: &[(&str, &str)] = &[("sonarr", "sonarrs"), ("radarr", "radarrs")];

/// Radarr alone: the stack before Sonarr wrote its key.
const RADARR: &[(&str, &str)] = &[("radarr", "radarrs")];

/// A stack whose gate holds nothing has a key minted for it, and is handed a route for
/// each \*arr, with the key it wrote, and one for Jellyfin, with the new key.
#[tokio::test]
async fn the_routes_are_written_with_a_key_of_the_gates_own() {
    let http = serving(&[&[], &[], &["fresh"]], 204, 204);
    let (ctx, at) = gate_ctx("gate-routes-minted", true, ARRS, None, http.clone());

    let (state, revoked) = seeded(&ctx, &http, true, &at).await;

    assert_eq!(state, Some(State::Wired));
    assert_eq!(routes(&at), Some(holding(ARRS, "fresh")));
    // The test stack pins Jellyfin at tag `1`, so the gate forwards to major 1 alone.
    assert_eq!(
        routes(&at).and_then(|held| held.route("jellyfin").map(|one| one.majors.clone())),
        Some(vec![1])
    );
    assert!(revoked.is_empty(), "{revoked:?}");
    assert!(http.asked_for(&format!("/Auth/Keys?App={APP}")));
}

/// Routes that already say what they should, under a key Jellyfin lists, are left alone.
#[tokio::test]
async fn routes_that_hold_are_left_alone() {
    let http = serving(&[&["kept"]], 204, 204);
    let (ctx, at) = gate_ctx("gate-routes-kept", true, ARRS, Some("kept"), http.clone());

    let (state, revoked) = seeded(&ctx, &http, true, &at).await;

    assert_eq!(state, Some(State::AlreadyWired));
    assert!(revoked.is_empty(), "{revoked:?}");
}

/// An \*arr that wrote a new key reaches the gate on the next pass, under the same
/// Jellyfin key, and a key filed under the gate's name that the routes do not hold is
/// revoked.
#[tokio::test]
async fn a_moved_arr_key_is_written_and_strays_revoked() {
    let http = serving(&[&["stray", "kept"]], 204, 204);
    let (ctx, at) = gate_ctx("gate-routes-moved", true, ARRS, Some("kept"), http.clone());
    let moved = [("sonarr", "regenerated"), ("radarr", "radarrs")];
    let _ = store::write(
        &at.join("config/sonarr/config.xml"),
        "<Config><ApiKey>regenerated</ApiKey></Config>",
    );

    let (state, revoked) = seeded(&ctx, &http, true, &at).await;

    assert_eq!(state, Some(State::Wired));
    assert_eq!(routes(&at), Some(holding(&moved, "kept")));
    assert_eq!(revoked, vec!["stray".to_owned()]);
}

/// An \*arr that has written no key yet has no route until it has.
#[tokio::test]
async fn an_arr_without_a_key_has_no_route() {
    let http = serving(&[&[], &[], &["fresh"]], 204, 204);
    let (ctx, at) = gate_ctx("gate-routes-keyless", true, RADARR, None, http.clone());

    let (state, _) = seeded(&ctx, &http, true, &at).await;

    assert_eq!(state, Some(State::Wired));
    assert_eq!(routes(&at), Some(holding(RADARR, "fresh")));
}

/// A key the routes hold that Jellyfin no longer lists is replaced, and what was filed
/// before the replacement is revoked once the routes are written.
#[tokio::test]
async fn a_key_jellyfin_dropped_is_replaced() {
    let http = serving(&[&["stray"], &["stray"], &["stray", "fresh"]], 204, 204);
    let (ctx, at) = gate_ctx(
        "gate-routes-dropped",
        true,
        ARRS,
        Some("gone"),
        http.clone(),
    );

    let (state, revoked) = seeded(&ctx, &http, true, &at).await;

    assert_eq!(state, Some(State::Wired));
    assert_eq!(routes(&at), Some(holding(ARRS, "fresh")));
    assert_eq!(revoked, vec!["stray".to_owned()]);
}

/// A stack that no longer runs the gate has every key filed under its name revoked; one
/// that never ran it, with nothing filed, says nothing.
#[tokio::test]
async fn removing_the_gate_revokes_its_key() {
    let http = serving(&[&["kept"]], 204, 204);
    let (ctx, at) = gate_ctx(
        "gate-routes-retired",
        true,
        ARRS,
        Some("kept"),
        http.clone(),
    );
    let (state, revoked) = seeded(&ctx, &http, false, &at).await;
    assert_eq!(state, Some(State::Wired));
    assert_eq!(revoked, vec!["kept".to_owned()]);

    let http = serving(&[&[]], 204, 204);
    let (ctx, at) = gate_ctx("gate-routes-none", true, ARRS, None, http.clone());
    assert_eq!(seeded(&ctx, &http, false, &at).await.0, None);
}

/// A rehearsal mints, writes and revokes nothing, and says what would change: the routes
/// by their names, never their credentials, or the keys where only they would.
#[tokio::test]
async fn a_rehearsal_mints_writes_and_revokes_nothing() {
    let routed = "routes to sonarr, radarr, jellyfin".to_owned();
    for (name, keys, held_now, moved, expected) in [
        (
            "gate-routes-would-mint",
            &[][..],
            None,
            false,
            State::WouldWire {
                yours: Some("no routes".to_owned()),
                ours: Some(routed.clone()),
            },
        ),
        (
            "gate-routes-would-move",
            &["kept"][..],
            Some("kept"),
            true,
            State::WouldWire {
                yours: Some("routes to radarr, jellyfin".to_owned()),
                ours: Some(routed.clone()),
            },
        ),
        (
            "gate-routes-would-revoke",
            &["stray", "kept"][..],
            Some("kept"),
            false,
            State::WouldWire {
                yours: Some(format!("2 keys filed as {APP}")),
                ours: Some(format!("one key filed as {APP}")),
            },
        ),
    ] {
        let http = serving(&[keys], 204, 204);
        let written = if moved { RADARR } else { ARRS };
        let (mut ctx, at) = gate_ctx(name, true, written, held_now, http.clone());
        if moved {
            let _ = store::write(
                &at.join("config/sonarr/config.xml"),
                "<Config><ApiKey>sonarrs</ApiKey></Config>",
            );
        }
        ctx.dry_run = true;
        let before = routes(&at);

        let (state, revoked) = seeded(&ctx, &http, true, &at).await;

        assert_eq!(state, Some(expected), "{name}");
        assert!(revoked.is_empty(), "{name}: {revoked:?}");
        assert!(
            !http
                .requests()
                .iter()
                .any(|asked| asked.method == Method::Post && asked.url.contains("/Auth/Keys")),
            "{name}"
        );
        assert_eq!(routes(&at), before, "{name}");
    }
}

/// A rehearsal before the first run says it would mint the key; a server lemonfiber does
/// not administer is left alone, and so is a stack without the gate.
#[tokio::test]
async fn without_an_administrator_only_a_rehearsal_says_anything() {
    let http = serving(&[&[]], 204, 204);
    let (mut ctx, at) = gate_ctx(
        "gate-routes-unadministered",
        false,
        ARRS,
        None,
        http.clone(),
    );

    assert_eq!(seeded(&ctx, &http, true, &at).await.0, None);
    ctx.dry_run = true;
    assert_eq!(seeded(&ctx, &http, false, &at).await.0, None);
    assert_eq!(
        seeded(&ctx, &http, true, &at).await.0,
        Some(State::WouldWire {
            yours: None,
            ours: None,
        })
    );
    assert!(http.requests().is_empty());
}

/// A stack with no Jellyfin has no routes to hand over.
#[tokio::test]
async fn a_stack_without_jellyfin_has_no_routes() {
    let http = serving(&[&[]], 204, 204);
    let (ctx, _) = gate_ctx("gate-routes-no-jellyfin", true, ARRS, None, http.clone());
    let services = vec![manifest_service(
        "request-gate",
        None,
        Some(lemonfiber_sidecar::gate::PORT),
    )];

    assert!(super::super::gate::seed_gate_routes(&ctx, &services, None)
        .await
        .is_none());
}

/// With no stack directory there is nowhere to hand the routes over, and nothing is
/// minted.
#[tokio::test]
async fn without_a_stack_directory_nothing_is_minted() {
    let http = serving(&[&[]], 204, 204);
    let (ctx, _) = gate_ctx("gate-routes-no-project", true, ARRS, None, http.clone());

    let wiring = super::super::gate::seed_gate_routes(&ctx, &stack(true), None).await;

    assert_eq!(
        wiring.map(|one| one.state),
        Some(State::Skipped {
            reason: "there is no stack directory to hand the request gate its routes in".to_owned(),
        })
    );
    assert!(!http
        .requests()
        .iter()
        .any(|asked| asked.method == Method::Post && asked.url.contains("/Auth/Keys")));
}

/// Jellyfin refusing the key list, the mint or a revocation is reported, never called done.
#[tokio::test]
async fn a_refusal_is_reported() {
    let unlisted = Fake::by_route(vec![
        (
            Method::Post,
            "/Users/AuthenticateByName",
            Answer::reply(200, r#"{"AccessToken":"token"}"#),
        ),
        (Method::Get, "/Auth/Keys", Answer::reply(500, "")),
    ]);
    let (ctx, at) = gate_ctx("gate-routes-unlisted", true, ARRS, None, unlisted.clone());
    let (state, _) = seeded(&ctx, &unlisted, true, &at).await;
    assert!(matches!(state, Some(State::Failed { .. })), "{state:?}");
    // Without the gate, an unreadable key list is nothing this stack asked about.
    assert_eq!(seeded(&ctx, &unlisted, false, &at).await.0, None);

    for (name, lists, held_now, minted, revoked) in [
        ("gate-routes-unminted", &[&[][..]][..], None, 500, 204),
        (
            "gate-routes-unrevoked",
            &[&["stray", "kept"][..]][..],
            Some("kept"),
            204,
            500,
        ),
        (
            "gate-routes-unrevoked-after",
            &[&["stray"][..], &["stray"][..], &["stray", "fresh"][..]][..],
            None,
            204,
            500,
        ),
    ] {
        let http = serving(lists, minted, revoked);
        let (ctx, at) = gate_ctx(name, true, ARRS, held_now, http.clone());

        let (state, _) = seeded(&ctx, &http, true, &at).await;

        assert!(
            matches!(state, Some(State::Failed { .. })),
            "{name}: {state:?}"
        );
    }
}

/// A minted key whose routes cannot be written is revoked again at once, so nothing is
/// left on the server that nothing holds.
#[tokio::test]
async fn a_key_whose_routes_cannot_be_written_is_revoked_again() {
    let http = serving(&[&[], &[], &["fresh"]], 204, 204);
    let (ctx, at) = gate_ctx("gate-routes-unwritable", true, ARRS, None, http.clone());
    blocked(&at);

    let (state, revoked) = seeded(&ctx, &http, true, &at).await;

    assert_eq!(
        state,
        Some(State::Failed {
            detail: format!(
                "the routes could not be written to {}, so the new key was revoked again: {}",
                routes_file(&at).display(),
                unwritable(&at)
            ),
        })
    );
    assert_eq!(revoked, vec!["fresh".to_owned()]);
}

/// Routes that cannot be brought up to date under a key the gate holds are said, and
/// the key stays: it is still the one the gate presents.
#[tokio::test]
async fn routes_that_cannot_be_updated_are_said() {
    let http = serving(&[&["stray", "kept"]], 204, 204);
    let (ctx, at) = gate_ctx(
        "gate-routes-unwritable-kept",
        true,
        ARRS,
        None,
        http.clone(),
    );
    blocked(&at);
    let outdated = holding(RADARR, "kept").written();
    let ctx = ctx.with_filesystem(lemonfiber_fixtures::files::Files::at(vec![
        (
            at.join("config/sonarr/config.xml"),
            "<Config><ApiKey>sonarrs</ApiKey></Config>",
        ),
        (
            at.join("config/radarr/config.xml"),
            "<Config><ApiKey>radarrs</ApiKey></Config>",
        ),
        (routes_file(&at), &outdated),
    ]));

    let (state, revoked) = seeded(&ctx, &http, true, &at).await;

    assert_eq!(
        state,
        Some(State::Failed {
            detail: format!(
                "the routes could not be written to {}: {}",
                routes_file(&at).display(),
                unwritable(&at)
            ),
        })
    );
    assert!(revoked.is_empty(), "{revoked:?}");
}

/// Put a directory where the routes file belongs under `project`, leaving nowhere to
/// write it.
fn blocked(project: &std::path::Path) {
    let _ = std::fs::create_dir_all(routes_file(project).join("blocked"));
}

/// What writing the routes file under a blocked `project` fails with.
fn unwritable(project: &std::path::Path) -> String {
    store::write(&routes_file(project), "")
        .err()
        .map(|failure| failure.to_string())
        .unwrap_or_default()
}
