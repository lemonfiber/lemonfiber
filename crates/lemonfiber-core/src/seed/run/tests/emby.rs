//! Emby as the media server: the request gate handed an Emby route in place of the Jellyfin
//! route, and the request service signed in through it in Emby's protocol.

use super::*;
use crate::app::gating::{EMBY_API, PORT};
use crate::ports::service::{Endpoint, IdentitySource, Protocol};
use crate::test_support::embying::{embied, emby_gate, PINNED, SERVER};
use crate::test_support::{media, requests};

/// The key Emby mints for the gate over `media.serve`.
const KEY: &str = "the-gates-emby-key";

/// The internal TLS port Emby declares.
const INTERNAL_TLS: u16 = 8920;

/// The administrator's password recorded for Emby before the run.
const RECORDED: &str = "minted-earlier";

/// The stack's request service and request gate, with no media server of its own.
fn services() -> Vec<lemonfiber_manifest::Service> {
    vec![requests_svc(), request_gate()]
}

/// The stack's services, with Emby moved into its plugin, the request service moved into
/// its plugin and the gate asking the identity source in Emby's API, written at `project`.
fn fillers(project: &std::path::Path) -> crate::wiring::Fillers {
    let installed: Vec<_> = embied()
        .into_iter()
        .chain(REQUESTS.installed())
        .chain([emby_gate()])
        .collect();
    fillers_trusting(
        services(),
        &installed,
        project,
        &beside_the_bundle(&GATE_TRUSTED),
    )
}

/// The media server [`fillers`] settles on.
fn server(project: &std::path::Path) -> Option<crate::app::targets::MediaServer> {
    crate::app::targets::MediaServer::of(&fillers(project))
}

/// What Emby declares: a guard's terms, and its internal TLS port.
fn declared() -> crate::ports::service::Declared {
    let mut declared = lemonfiber_fixtures::declaring::enforceable();
    declared.ports.internal_tls = Some(INTERNAL_TLS);
    declared
}

/// A stack directory named `name`, its env file recording [`RECORDED`] as Emby's
/// administrator's password.
fn project(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let at = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(&at);
    let env = at.join(".env");
    let _ = std::fs::write(&env, "DATA_ROOT=/srv/media\n");
    if let Some(server) = server(&at) {
        crate::test_support::recorded_as(&env, &server.setting, RECORDED);
    }
    (at, env)
}

/// A context over the stack at `at`, its settings beside `env`, beside `gate`, asking Emby
/// and the request service through their adapters over `http`.
fn emby_ctx(
    (at, env): (&std::path::Path, std::path::PathBuf),
    gate: Gate,
    http: Arc<Fake>,
) -> (Ctx, Arc<Gate>) {
    let ctx = a_context()
        .environment(crate::platform::Environment::LinuxNative)
        .settings(Settings {
            env_file: Some(env),
            ..Settings::default()
        })
        .build()
        .with_http(http)
        .with_random(Arc::new(FixedRandom(Some(vec![
            0xab;
            crate::secret::SECRET_BYTES
        ]))));
    let gate = Arc::new(gate);
    (
        crate::test_support::gated(
            crate::test_support::adapted::asking(ctx, &[media::MOVED, REQUESTS]),
            at,
            &gate,
        ),
        gate,
    )
}

/// The gate's routes once seeded over `ctx`, and the state the step rests in.
async fn routed(ctx: &Ctx, project: &std::path::Path) -> Option<State> {
    let fillers = fillers(project);
    let server = crate::app::targets::MediaServer::of(&fillers);
    super::super::gate::seed_gate_routes(ctx, &services(), &fillers, server.as_ref())
        .await
        .into_iter()
        .next()
        .map(|wiring| wiring.state)
}

#[test]
fn emby_is_the_media_server_the_gate_routes_in_embys_own_api() {
    let server = server(stack_root());

    assert_eq!(server.as_ref().map(|one| one.id()), Some(SERVER));
    assert_eq!(
        server
            .as_ref()
            .and_then(|one| one.gated())
            .map(|pairing| pairing.protocol.clone()),
        Some(Protocol(EMBY_API.to_owned()))
    );
    assert!(
        server
            .as_ref()
            .is_some_and(|one| !one.plex() && !one.claimed()),
        "{server:?}"
    );
}

/// **The gate is handed an Emby route in place of the Jellyfin route.** It is answered
/// under `emby`, reaches Emby's internal TLS port pinned to the certificate kept for it,
/// forwards to the majors of Emby the stack supports, and presents a key Emby minted for
/// the gate over `media.serve`.
#[tokio::test]
async fn emby_hands_the_gate_an_emby_route_under_a_key_minted_over_media_serve() {
    let http = Fake::by_path(vec![
        (media::serve("declared"), media::answered(&declared())),
        (
            media::serve("filed_as"),
            media::answered(&Vec::<String>::new()),
        ),
        (media::serve("mint"), media::answered(&KEY)),
        (media::serve("revoke"), media::taken()),
    ]);
    let (at, env) = project("emby-routed");
    let made = crate::certificate::internal::kept_or_made(
        &lemonfiber_adapters::Disk,
        &at.join("config").join(SERVER),
        SERVER,
    );
    let Ok(made) = made else {
        unreachable!("the internal certificate is made: {made:?}");
    };
    let (ctx, gate) = emby_ctx((&at, env), Gate::default(), http.clone());

    let state = routed(&ctx, &at).await;

    assert_eq!(state, Some(State::Wired));
    assert_eq!(
        gate.routes()
            .into_iter()
            .find(|route| to_the_media_server(&route.target)),
        Some(Gated {
            target: Target::Emby {
                pins: vec![made.fingerprint.clone()]
            },
            path: EMBY_API.to_owned(),
            address: format!("https://{SERVER}:{INTERNAL_TLS}"),
            majors: lemonfiber_manifest::majors(PINNED),
            credential: KEY.to_owned(),
            tokens: Vec::new(),
        })
    );
    assert!(http.asked_for(media::serve("mint")));
    assert!(!gate
        .routes()
        .iter()
        .any(|route| matches!(route.target, Target::MediaServer { .. })));
}

/// Whether a route reaching `target` reaches the media server.
fn to_the_media_server(target: &Target) -> bool {
    crate::app::gating::media_pins(target).is_some()
}

/// Emby set up, and a request service answering each of `initialised` in turn when asked
/// whether it is set up, linked nowhere yet, taking every write.
fn signing(initialised: &[bool]) -> Arc<Fake> {
    Fake::by_path_in_turn(vec![
        (
            media::identity("startup_completed"),
            vec![media::answered(&true)],
        ),
        (
            media::identity("replace_admin_password"),
            vec![media::taken()],
        ),
        (media::serve("filed_as"), vec![media::answered(&[KEY])]),
        (requests::intake("establish"), vec![requests::taken()]),
        (
            requests::intake("configure_identity"),
            vec![requests::taken()],
        ),
        (
            requests::intake("initialized"),
            initialised.iter().map(requests::answered).collect(),
        ),
        (
            requests::intake("media_server_link"),
            vec![requests::answered(
                &crate::ports::service::MediaServerLink {
                    at: Endpoint {
                        host: String::new(),
                        port: 0,
                        base: String::new(),
                    },
                    key: String::new(),
                },
            )],
        ),
        (
            requests::intake("link_media_server"),
            vec![requests::taken()],
        ),
        (
            requests::intake("telling"),
            vec![requests::answered(&crate::seed::wanted_telling())],
        ),
        (requests::intake("tell"), vec![requests::taken()]),
    ])
}

/// The gate holding its Emby route under [`KEY`].
fn holding_emby() -> Gate {
    Gate::admitting(vec![Gated {
        target: Target::Emby { pins: Vec::new() },
        path: EMBY_API.to_owned(),
        address: format!("https://{SERVER}:{INTERNAL_TLS}"),
        majors: lemonfiber_manifest::majors(PINNED),
        credential: KEY.to_owned(),
        tokens: Vec::new(),
    }])
}

/// **The request service signs the household in through the gate's Emby route, in Emby's
/// protocol.** `establish` and `configure_identity` each carry an identity source naming
/// the protocol `emby` and the gate's Emby route, and the request service is then pointed
/// at that route.
#[tokio::test]
async fn the_request_service_is_set_up_through_the_gates_emby_route_in_embys_protocol() {
    let http = signing(&[false, true]);
    let (at, env) = project("emby-signed");
    let (ctx, _) = emby_ctx((&at, env), holding_emby(), http.clone());
    let fillers = fillers(&at);
    let server = crate::app::targets::MediaServer::of(&fillers);
    let admin = super::super::identity::seed_media_server_admin(&ctx, server.as_ref()).await;

    let (wirings, _) = super::super::seed_request_identity(
        &ctx,
        &services(),
        &fillers,
        &crate::baseline::Baseline::new(),
        server.as_ref(),
        admin,
    )
    .await;

    assert!(
        wirings
            .iter()
            .any(|one| one.connection == crate::seed::IDENTITY && one.state == State::Wired),
        "{wirings:?}"
    );
    let through_the_gate = format!("http://{}:{PORT}/{EMBY_API}", crate::app::gating::SERVICE);
    for operation in ["establish", "configure_identity"] {
        let sources = requests::told::<IdentitySource>(&http.requests(), operation, "source");
        assert_eq!(
            sources
                .iter()
                .map(|source| (source.protocol.clone(), source.at.clone()))
                .collect::<Vec<_>>(),
            vec![(Protocol(EMBY_API.to_owned()), through_the_gate.clone())],
            "{operation}"
        );
    }
    assert_eq!(
        requests::told::<Endpoint>(&http.requests(), "link_media_server", "at"),
        vec![Endpoint {
            host: crate::app::gating::SERVICE.to_owned(),
            port: PORT,
            base: format!("/{EMBY_API}"),
        }]
    );
}
