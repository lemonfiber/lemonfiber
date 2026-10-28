//! The request gate's routes: each upstream it answers for, and the key it holds there.

use lemonfiber_sidecar::gate::{Credential, File, Kind, Upstream, Upstreams};

use super::*;
use crate::test_support::media;

/// The name the gate's media server key is filed under.
const APP: &str = crate::app_keys::GATE_APP;

/// A stack with the media server, the request service and TV, movie and music curators —
/// the music one files music the request service never asks for, so the gate has no route
/// to it — and, where `gating`, the request gate.
fn stack(gating: bool) -> Vec<lemonfiber_manifest::Service> {
    let mut services = vec![
        media_server_svc(),
        requests_svc(),
        curator("sonarr", 8989, "tv"),
        curator("radarr", 7878, "movies"),
        curator("lidarr", 8686, "music"),
    ];
    if gating {
        services.push(request_gate());
    }
    services
}

/// The parts every route here is reached through, moved into their plugins: the media
/// server and the request service.
fn moved_parts() -> Vec<crate::plugin::Installed> {
    media::moved()
        .into_iter()
        .chain(crate::test_support::requests::REQUESTS.installed())
        .collect()
}

/// `services` written at `project`, with [`moved_parts`] and each of its curators moved.
fn routed_at(
    services: Vec<lemonfiber_manifest::Service>,
    project: &std::path::Path,
) -> crate::wiring::Fillers {
    let installed: Vec<crate::plugin::Installed> = moved_parts()
        .into_iter()
        .chain(curators_moved(&services))
        .collect();
    fillers_beside(services, &installed, project)
}

/// `key` kept in `ctx`'s secret store as the key lemonfiber minted for the curator
/// `curator` moved into its plugin.
fn minted_for(ctx: &Ctx, curator: &str, key: &str) {
    crate::test_support::curators::ALL
        .iter()
        .filter(|moved| moved.plugin == curator)
        .for_each(|moved| moved.minted(ctx, key));
}

/// The internal TLS port the media server declares.
const INTERNAL_TLS: u16 = 8920;

/// What the media server declares: a guard's terms, and its internal TLS port.
fn declared() -> crate::ports::service::Declared {
    let mut declared = lemonfiber_fixtures::declaring::enforceable();
    declared.ports.internal_tls = Some(INTERNAL_TLS);
    declared
}

/// A media server filing `filed` under the gate's name and minting `minted`, taking a mint
/// where `mints` and a revocation where `revokes`.
fn serving(filed: &[&str], minted: &str, mints: bool, revokes: bool) -> Arc<Fake> {
    Fake::by_path(vec![
        (media::serve("declared"), media::answered(&declared())),
        (media::serve("filed_as"), media::answered(&filed)),
        (
            media::serve("mint"),
            if mints {
                media::answered(&minted)
            } else {
                media::refusing()
            },
        ),
        (
            media::serve("revoke"),
            if revokes {
                media::taken()
            } else {
                media::refusing()
            },
        ),
    ])
}

/// Whether `http` was asked to mint a key.
fn minted_on(http: &Fake) -> bool {
    http.asked_for(media::serve("mint"))
}

/// The majors of the API the moved media server answers in, which the gate forwards to.
fn moved_majors() -> Vec<u32> {
    crate::app::targets::MediaServer::of(&routed_at(stack(true), stack_root()))
        .and_then(|server| server.pairing)
        .map(|pairing| pairing.majors)
        .unwrap_or_default()
}

/// A stack directory holding the key lemonfiber minted for each of `curators`, and `held`
/// as the gate's media server key where it holds one, and a context asking the media
/// server moved into its plugin as its administrator where `administered` says, answering
/// over `http`.
fn gate_ctx(
    name: &str,
    administered: bool,
    curators: &[(&str, &str)],
    held: Option<&str>,
    http: Arc<Fake>,
) -> (Ctx, std::path::PathBuf) {
    let at = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(&at);
    let env = at.join(".env");
    let _ = std::fs::write(&env, "DATA_ROOT=/srv/media\n");
    if let Some(held) = held {
        let _ = store::write(&routes_file(&at), &holding(curators, held).written());
    }
    let ctx = a_context()
        .environment(crate::platform::Environment::LinuxNative)
        .settings(Settings {
            env_file: Some(env),
            ..Settings::default()
        })
        .build()
        .with_http(http);
    for (curator, key) in curators {
        minted_for(&ctx, curator, key);
    }
    (
        if administered {
            media::asking(ctx)
        } else {
            ctx
        },
        at,
    )
}

/// The routes the gate should hold where lemonfiber minted each of `curators` its key and
/// `key` is its media server key.
fn holding(curators: &[(&str, &str)], key: &str) -> Upstreams {
    let mut upstreams: Vec<Upstream> = curators
        .iter()
        .map(|(curator, credential)| Upstream {
            route: (*curator).to_owned(),
            kind: if *curator == "sonarr" {
                Kind::Sonarr
            } else {
                Kind::Radarr
            },
            address: format!(
                "http://{curator}:{}",
                if *curator == "sonarr" { 8989 } else { 7878 }
            ),
            credential: Credential::new(*credential),
            majors: Vec::new(),
            pins: lemonfiber_sidecar::Pins::default(),
        })
        .collect();
    upstreams.push(Upstream {
        route: "jellyfin".to_owned(),
        kind: Kind::Jellyfin,
        address: media_server_svc_network_url(),
        credential: Credential::new(key),
        majors: moved_majors(),
        pins: lemonfiber_sidecar::Pins::default(),
    });
    Upstreams::of(upstreams)
}

/// Where the gate reaches the media server: at the id its plugin's server keeps on the
/// stack's network, on the port it listens on there.
fn media_server_svc_network_url() -> String {
    format!("https://jellyfin:{INTERNAL_TLS}")
}

/// Where the gate reads its routes, under `project`.
fn routes_file(project: &std::path::Path) -> std::path::PathBuf {
    crate::app::gating::path(project, File::Upstreams)
}

/// What the routes file under `project` holds, where it reads.
pub(super) fn routes(project: &std::path::Path) -> Option<Upstreams> {
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
    let fillers = routed_at(stack(gating), project);
    let server = crate::app::targets::MediaServer::of(&fillers);
    let wiring = super::super::gate::seed_gate_routes(
        ctx,
        &stack(gating),
        &fillers,
        server.as_ref(),
        Some(project),
    )
    .await;
    (
        wiring.into_iter().next().map(|one| one.state),
        revoked(http),
    )
}

/// Every key revoked over `http`.
fn revoked(http: &Fake) -> Vec<String> {
    http.requests()
        .into_iter()
        .filter(|asked| asked.url.ends_with(media::serve("revoke")))
        .filter_map(|asked| serde_json::from_str::<serde_json::Value>(asked.body.as_deref()?).ok())
        .filter_map(|asked| asked.get("key")?.as_str().map(str::to_owned))
        .collect()
}

/// Both curators, with the keys lemonfiber minted for them.
const CURATORS: &[(&str, &str)] = &[("sonarr", "sonarrs"), ("radarr", "radarrs")];

/// The movie curator alone: the stack before lemonfiber minted the TV curator a key.
const MOVIES_ONLY: &[(&str, &str)] = &[("radarr", "radarrs")];

/// A stack whose gate holds nothing has a key minted for it, and is handed a route for
/// each curator, with the key lemonfiber minted for it, and one for the media server, with
/// the new key.
#[tokio::test]
async fn the_routes_are_written_with_a_key_of_the_gates_own() {
    let http = serving(&[], "fresh", true, true);
    let (ctx, at) = gate_ctx("gate-routes-minted", true, CURATORS, None, http.clone());

    let (state, revoked) = seeded(&ctx, &http, true, &at).await;

    assert_eq!(state, Some(State::Wired));
    assert_eq!(routes(&at), Some(holding(CURATORS, "fresh")));
    // The gate forwards to the majors of the API the plugin's server answers in, and to
    // nothing else.
    assert!(!moved_majors().is_empty());
    assert_eq!(
        routes(&at).and_then(|held| held.route("jellyfin").map(|one| one.majors.clone())),
        Some(moved_majors())
    );
    assert!(revoked.is_empty(), "{revoked:?}");
    assert!(http
        .requests()
        .iter()
        .any(|asked| asked.url.ends_with(media::serve("filed_as"))
            && asked.body.as_deref() == Some(format!(r#"{{"app":"{APP}"}}"#).as_str())));
}

/// Routes that already say what they should, under a key the media server lists, are left
/// alone.
#[tokio::test]
async fn routes_that_hold_are_left_alone() {
    let http = serving(&["kept"], "fresh", true, true);
    let (ctx, at) = gate_ctx(
        "gate-routes-kept",
        true,
        CURATORS,
        Some("kept"),
        http.clone(),
    );

    let (state, revoked) = seeded(&ctx, &http, true, &at).await;

    assert_eq!(state, Some(State::AlreadyWired));
    assert!(revoked.is_empty(), "{revoked:?}");
}

/// A curator key minted anew reaches the gate on the next pass, under the same media
/// server key, and a key filed under the gate's name that the routes do not hold is
/// revoked.
#[tokio::test]
async fn a_moved_curator_key_is_written_and_strays_revoked() {
    let http = serving(&["stray", "kept"], "fresh", true, true);
    let (ctx, at) = gate_ctx(
        "gate-routes-moved",
        true,
        CURATORS,
        Some("kept"),
        http.clone(),
    );
    let moved = [("sonarr", "regenerated"), ("radarr", "radarrs")];
    minted_for(&ctx, "sonarr", "regenerated");

    let (state, revoked) = seeded(&ctx, &http, true, &at).await;

    assert_eq!(state, Some(State::Wired));
    assert_eq!(routes(&at), Some(holding(&moved, "kept")));
    assert_eq!(revoked, vec!["stray".to_owned()]);
}

/// Every line the gate's pass reports beside its own, by connection and state.
async fn unrouted(ctx: &Ctx, project: &std::path::Path) -> Vec<(String, State)> {
    let fillers = routed_at(stack(true), project);
    let server = crate::app::targets::MediaServer::of(&fillers);
    super::super::gate::seed_gate_routes(
        ctx,
        &stack(true),
        &fillers,
        server.as_ref(),
        Some(project),
    )
    .await
    .into_iter()
    .skip(1)
    .map(|one| (one.connection, one.state))
    .collect()
}

/// A curator lemonfiber holds no minted key for has no route, and the pass says it is
/// skipped.
#[tokio::test]
async fn a_curator_holding_no_minted_key_has_no_route() {
    let http = serving(&[], "fresh", true, true);
    let (ctx, at) = gate_ctx("gate-routes-keyless", true, MOVIES_ONLY, None, http.clone());

    let said = unrouted(&ctx, &at).await;

    assert_eq!(routes(&at), Some(holding(MOVIES_ONLY, "fresh")));
    assert_eq!(
        said,
        vec![(
            "Sonarr routed through the request gate".to_owned(),
            State::Skipped {
                reason: "lemonfiber holds no key it minted for Sonarr; a run once it holds one \
                         completes it"
                    .to_owned()
            }
        )]
    );
}

/// A key the routes hold that the media server no longer lists is replaced, and what was
/// filed before the replacement is revoked once the routes are written.
#[tokio::test]
async fn a_key_the_media_server_dropped_is_replaced() {
    let http = serving(&["stray"], "fresh", true, true);
    let (ctx, at) = gate_ctx(
        "gate-routes-dropped",
        true,
        CURATORS,
        Some("gone"),
        http.clone(),
    );

    let (state, revoked) = seeded(&ctx, &http, true, &at).await;

    assert_eq!(state, Some(State::Wired));
    assert_eq!(routes(&at), Some(holding(CURATORS, "fresh")));
    assert_eq!(revoked, vec!["stray".to_owned()]);
}

/// A stack that no longer runs the gate has every key filed under its name revoked; one
/// that never ran it, with nothing filed, says nothing.
#[tokio::test]
async fn removing_the_gate_revokes_its_key() {
    let http = serving(&["kept"], "fresh", true, true);
    let (ctx, at) = gate_ctx(
        "gate-routes-retired",
        true,
        CURATORS,
        Some("kept"),
        http.clone(),
    );
    let (state, revoked) = seeded(&ctx, &http, false, &at).await;
    assert_eq!(state, Some(State::Wired));
    assert_eq!(revoked, vec!["kept".to_owned()]);

    let http = serving(&[], "fresh", true, true);
    let (ctx, at) = gate_ctx("gate-routes-none", true, CURATORS, None, http.clone());
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
        let http = serving(keys, "fresh", true, true);
        let written = if moved { MOVIES_ONLY } else { CURATORS };
        let (mut ctx, at) = gate_ctx(name, true, written, held_now, http.clone());
        if moved {
            minted_for(&ctx, "sonarr", "sonarrs");
        }
        ctx.dry_run = true;
        let before = routes(&at);

        let (state, revoked) = seeded(&ctx, &http, true, &at).await;

        assert_eq!(state, Some(expected), "{name}");
        assert!(revoked.is_empty(), "{name}: {revoked:?}");
        assert!(!minted_on(&http), "{name}");
        assert_eq!(routes(&at), before, "{name}");
    }
}

/// A rehearsal before the first run says it would mint the key; a server lemonfiber does
/// not administer is left alone, and so is a stack without the gate.
#[tokio::test]
async fn without_an_administrator_only_a_rehearsal_says_anything() {
    let http = serving(&[], "fresh", true, true);
    let (mut ctx, at) = gate_ctx(
        "gate-routes-unadministered",
        false,
        CURATORS,
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

/// A stack with no media server the gate answers for still has a route to each curator
/// the request service fulfils through, and nothing is asked of any server.
#[tokio::test]
async fn without_a_media_server_the_curators_are_still_routed() {
    let http = serving(&[], "fresh", true, true);
    let curators = [("sonarr", "sonarr-key")];
    let (ctx, project) = gate_ctx(
        "gate-routes-no-media-server",
        true,
        &curators,
        None,
        http.clone(),
    );
    let services = vec![
        requests_svc(),
        curator("sonarr", 8989, "tv"),
        request_gate(),
    ];

    let wiring = super::super::gate::seed_gate_routes(
        &ctx,
        &services,
        &routed_at(services.clone(), &project),
        served(&services).as_ref(),
        Some(&project),
    )
    .await;

    let mut expected = holding(&curators, "").upstreams;
    expected.retain(|route| route.kind != Kind::Jellyfin);
    assert_eq!(
        wiring.into_iter().next().map(|one| one.state),
        Some(State::Wired)
    );
    assert_eq!(routes(&project), Some(Upstreams::of(expected)));
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

/// What the gate's routes come to where only `services` stand, under `project` where
/// one is given.
async fn curators_seeded(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    fillers: &crate::wiring::Fillers,
    project: Option<&std::path::Path>,
) -> Option<State> {
    super::super::gate::seed_gate_routes(ctx, services, fillers, served(services).as_ref(), project)
        .await
        .into_iter()
        .next()
        .map(|one| one.state)
}

/// Without a media server the gate answers for, a rehearsal says what it would write and
/// writes nothing, a run writes the curators' routes and the next leaves them alone, a
/// missing stack directory is said, and routes that cannot be written are reported.
#[tokio::test]
async fn the_curators_routes_alone_are_rehearsed_held_and_reported() {
    let http = serving(&[], "fresh", true, true);
    let curators = [("sonarr", "sonarr-key")];
    let (mut ctx, project) = gate_ctx(
        "gate-routes-curators-alone",
        true,
        &curators,
        None,
        http.clone(),
    );
    let services = vec![
        requests_svc(),
        curator("sonarr", 8989, "tv"),
        request_gate(),
    ];
    let fillers = routed_at(services.clone(), &project);
    let mut expected = holding(&curators, "").upstreams;
    expected.retain(|route| route.kind != Kind::Jellyfin);

    ctx.dry_run = true;
    let rehearsed = curators_seeded(&ctx, &services, &fillers, Some(&project)).await;
    ctx.dry_run = false;
    assert!(
        matches!(
            rehearsed,
            Some(State::WouldWire {
                yours: Some(_),
                ours: Some(_)
            })
        ),
        "{rehearsed:?}"
    );
    assert_eq!(routes(&project), None);

    assert_eq!(
        curators_seeded(&ctx, &services, &fillers, Some(&project)).await,
        Some(State::Wired)
    );
    assert_eq!(
        curators_seeded(&ctx, &services, &fillers, Some(&project)).await,
        Some(State::AlreadyWired)
    );
    assert_eq!(routes(&project), Some(Upstreams::of(expected)));

    assert_eq!(
        curators_seeded(&ctx, &services, &fillers, None).await,
        Some(State::Skipped {
            reason: "there is no stack directory to hand the request gate its routes in".to_owned(),
        })
    );

    let _ = std::fs::remove_file(routes_file(&project));
    let _ = std::fs::create_dir_all(routes_file(&project));
    let failed = curators_seeded(&ctx, &services, &fillers, Some(&project)).await;
    assert!(
        matches!(&failed, Some(State::Failed { detail })
            if detail.starts_with("the routes could not be handed to the request gate: ")),
        "{failed:?}"
    );
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

/// With no stack directory there is nowhere to hand the routes over, and nothing is
/// minted.
#[tokio::test]
async fn without_a_stack_directory_nothing_is_minted() {
    let http = serving(&[], "fresh", true, true);
    let (ctx, _) = gate_ctx("gate-routes-no-project", true, CURATORS, None, http.clone());

    let wiring = super::super::gate::seed_gate_routes(
        &ctx,
        &stack(true),
        &routed_at(stack(true), stack_root()),
        crate::app::targets::MediaServer::of(&routed_at(stack(true), stack_root())).as_ref(),
        None,
    )
    .await;

    assert_eq!(
        wiring.into_iter().next().map(|one| one.state),
        Some(State::Skipped {
            reason: "there is no stack directory to hand the request gate its routes in".to_owned(),
        })
    );
    assert!(!minted_on(&http));
}

/// The key the gate holds is told by its SHA-256, so a second key filed under its name
/// that shares its short fingerprint is not taken for it.
#[test]
fn the_key_the_gate_holds_is_told_from_one_sharing_its_short_fingerprint() {
    const HELD: &str = "000000000000031d";
    const TWIN: &str = "00000000000008d8";
    assert_eq!(
        crate::ports::fingerprint(HELD),
        crate::ports::fingerprint(TWIN)
    );
    let admitted = [crate::ports::service::Admitted::of(
        &crate::ports::service::Gated {
            target: crate::ports::service::Target::MediaServer { pins: Vec::new() },
            path: "jellyfin".to_owned(),
            address: "http://jellyfin:8096".to_owned(),
            majors: Vec::new(),
            credential: HELD.to_owned(),
            tokens: Vec::new(),
        },
    )];

    assert_eq!(
        super::super::gate::held(&admitted, &[TWIN.to_owned(), HELD.to_owned()]),
        Some(HELD.to_owned())
    );
    assert_eq!(
        super::super::gate::held(&admitted, &[TWIN.to_owned()]),
        None
    );
}

/// A gate the media server's key may not be handed to is given no route to the server,
/// and every key filed under its name there is revoked, since nothing holds it.
#[tokio::test]
async fn a_gate_the_media_servers_key_may_not_reach_holds_none() {
    let http = serving(&["stale"], "fresh", true, true);
    let (ctx, at) = gate_ctx("gate-withheld", true, CURATORS, None, http.clone());
    let mut services = stack(false);
    services.retain(|service| service.id != crate::app::gating::SERVICE);
    let installed: Vec<_> = moved_parts()
        .into_iter()
        .chain([crate::test_support::contracted(
            "stranger",
            crate::app::gating::SERVICE,
            lemonfiber_contract::capabilities::request::gate::CAPABILITY,
        )])
        .collect();
    let fillers = fillers_trusting(
        services,
        &installed,
        &at,
        crate::plugin::first_party::EMBEDDED,
    );
    let server = crate::app::targets::MediaServer::of(&fillers);

    let wirings = super::super::gate::seed_gate_routes(
        &ctx,
        &stack(true),
        &fillers,
        server.as_ref(),
        Some(&at),
    )
    .await;

    assert_eq!(revoked(&http), vec!["stale".to_owned()], "{wirings:?}");
    assert_eq!(routes(&at), None);
}

/// A stack that names no gate of its own asked for no route to the media server, even
/// beside a plugin's gate that may hold its key: every key filed under the gate's name is
/// revoked, and a key list the server will not give is left for a later run.
#[tokio::test]
async fn a_stack_naming_no_gate_revokes_its_key_beside_a_plugins() {
    let http = serving(&["stale"], "fresh", true, true);
    let (ctx, at) = gate_ctx("gate-unnamed", true, CURATORS, None, http.clone());
    let installed: Vec<_> = moved_parts()
        .into_iter()
        .chain([crate::test_support::a_contracted_gate()])
        .collect();
    let trusted: Vec<_> = crate::test_support::GATE_TRUSTED
        .into_iter()
        .chain(crate::plugin::first_party::EMBEDDED.iter().copied())
        .collect();
    let fillers = fillers_trusting(stack(false), &installed, &at, &trusted);
    let server = crate::app::targets::MediaServer::of(&fillers);

    let wirings = super::super::gate::seed_gate_routes(
        &ctx,
        &stack(false),
        &fillers,
        server.as_ref(),
        Some(&at),
    )
    .await;

    let unlisted = Fake::by_path(vec![(media::serve("filed_as"), media::refusing())]);
    let unread = super::super::gate::seed_gate_routes(
        &ctx.with_http(unlisted.clone()),
        &stack(false),
        &fillers,
        server.as_ref(),
        Some(&at),
    )
    .await;

    assert_eq!(
        wirings.into_iter().map(|one| one.state).collect::<Vec<_>>(),
        vec![State::Wired]
    );
    assert_eq!(revoked(&http), vec!["stale".to_owned()]);
    assert_eq!(routes(&at), None);
    assert!(unread.is_empty(), "{unread:?}");
    assert!(unlisted.asked_for(media::serve("filed_as")));
}

mod refusing;

mod pinning;

mod rotating;
