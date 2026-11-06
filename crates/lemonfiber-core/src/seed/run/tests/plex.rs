//! The request service linked to Plex in the mode `PLEX_LINK_MODE` names, through the request
//! gate's Plex route.

use lemonfiber_contract::capabilities::request::intake;
use lemonfiber_contract::Contracted;

use super::*;
use crate::ports::digest::sha256;
use crate::ports::service::{OutsideAccount, OutsideServer, PlexLink, PlexMember};
use crate::test_support::media;

/// The request service's own managed account token, filed under the gate's name.
const KEY: &str = "the-managed-account-token";

/// The internal TLS port Plex declares.
const INTERNAL_TLS: u16 = 32443;

/// The plex.tv id of the account that claimed Plex.
const CLAIMED_BY: &str = "4815162342";

/// The media server's id, which names the gate's route to it.
const SERVER: &str = "jellyfin";

/// Where the host reaches the request service a test asks.
const REQUESTS_AT: &str = "http://127.0.0.1:8080";

/// Plex moved into its plugin, beside the request service moved into its plugin.
fn plexed() -> Vec<crate::plugin::Installed> {
    crate::test_support::plexing::plexed()
        .into_iter()
        .chain(REQUESTS.installed())
        .collect()
}

/// The request gate, asking the identity source in Jellyfin's API or Plex's.
fn plex_gate() -> crate::plugin::Installed {
    crate::test_support::plexing::asking_in_plex_too(a_contracted_gate())
}

/// The stack's media server, request service and request gate, written at `project`, with
/// [`plexed`] and [`plex_gate`] beside it.
fn fillers(project: &std::path::Path) -> crate::wiring::Fillers {
    let installed: Vec<_> = plexed().into_iter().chain([plex_gate()]).collect();
    fillers_trusting(
        vec![media_server_svc(), requests_svc(), request_gate()],
        &installed,
        project,
        &beside_the_bundle(&GATE_TRUSTED),
    )
}

/// The media server [`fillers`] settles on.
fn server(project: &std::path::Path) -> Option<crate::app::targets::MediaServer> {
    crate::app::targets::MediaServer::of(&fillers(project))
}

/// A context over a stack directory beside `gate`, recording `mode` as the link mode where
/// it names one, asking the media server as its administrator over `http`.
fn plex_ctx(
    name: &str,
    mode: Option<&str>,
    gate: Gate,
    http: Arc<Fake>,
) -> (Ctx, std::path::PathBuf, Arc<Gate>) {
    let at = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(&at);
    let env = at.join(".env");
    let _ = std::fs::write(&env, "DATA_ROOT=/srv/media\n");
    if let Some(mode) = mode {
        assert!(crate::settings::store::recorded(
            &env.with_file_name(crate::config::paths::SETTINGS),
            &[(crate::plex_link::SETTING, Some(mode))].into(),
        )
        .is_ok());
    }
    let ctx = a_context()
        .environment(crate::platform::Environment::LinuxNative)
        .settings(Settings {
            env_file: Some(env),
            ..Settings::default()
        })
        .build()
        .with_http(http)
        .with_random(Arc::new(FixedRandom(Some(vec![0x11; 24]))));
    let gate = Arc::new(gate);
    (
        crate::test_support::gated(
            crate::test_support::adapted::asking(ctx, &[media::MOVED, REQUESTS]),
            &at,
            &gate,
        ),
        at,
        gate,
    )
}

/// What Plex declares: a guard's terms, and its internal TLS port.
fn declared() -> crate::ports::service::Declared {
    let mut declared = lemonfiber_fixtures::declaring::enforceable();
    declared.ports.internal_tls = Some(INTERNAL_TLS);
    declared
}

/// The request service's own Plex account.
fn own_account() -> OutsideAccount {
    OutsideAccount {
        id: "1001".to_owned(),
        uuid: "a1b2c3".to_owned(),
        username: "lemonfiber-gate".to_owned(),
        title: "lemonfiber-gate".to_owned(),
    }
}

/// Plex, as listed at plex.tv.
fn listed() -> OutsideServer {
    OutsideServer {
        name: "Living room".to_owned(),
        id: "0f1e2d3c".to_owned(),
    }
}

/// Plex over its contracts, filing `filed` under the gate's name, minting [`KEY`] and
/// claimed by `claimed_by`, with `extra` answered first.
fn plex(
    filed: &[&str],
    claimed_by: Option<&str>,
    extra: Vec<(&'static str, Vec<Answer>)>,
) -> Arc<Fake> {
    let mut routes = extra;
    routes.extend([
        (media::serve("declared"), vec![media::answered(&declared())]),
        (media::serve("filed_as"), vec![media::answered(&filed)]),
        (media::serve("mint"), vec![media::answered(&KEY)]),
        (media::serve("revoke"), vec![media::taken()]),
        (
            media::serve("account"),
            vec![media::answered(&own_account())],
        ),
        (media::serve("listed_as"), vec![media::answered(&listed())]),
        (
            media::identity("household"),
            vec![media::answered(&vec![media::member("4815", "Ann", true)])],
        ),
        (
            crate::test_support::adapted::claiming("claimed_by"),
            vec![media::answered(&claimed_by)],
        ),
    ]);
    Fake::by_path_in_turn(routes)
}

/// The gate's Plex route presenting [`KEY`] and admitting `tokens`.
fn plex_route(tokens: &[String]) -> Gated {
    Gated {
        target: Target::Plex {
            pins: Vec::new(),
            link: PlexLink::Managed,
        },
        path: SERVER.to_owned(),
        address: format!("https://{SERVER}:{INTERNAL_TLS}"),
        majors: Vec::new(),
        credential: KEY.to_owned(),
        tokens: tokens.to_vec(),
    }
}

/// The gate's routes once seeded over `ctx`, and the state the step rests in.
async fn routed(ctx: &Ctx, project: &std::path::Path) -> Option<State> {
    let fillers = fillers(project);
    let server = crate::app::targets::MediaServer::of(&fillers);
    let services = vec![media_server_svc(), requests_svc(), request_gate()];
    super::super::gate::seed_gate_routes(ctx, &services, &fillers, server.as_ref())
        .await
        .into_iter()
        .next()
        .map(|wiring| wiring.state)
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

#[test]
fn plex_is_the_media_server_the_gate_routes_in_plexs_own_api_and_is_claimed() {
    let server = server(stack_root());

    assert!(
        server
            .as_ref()
            .is_some_and(crate::app::targets::MediaServer::plex),
        "{server:?}"
    );
    assert!(
        server
            .as_ref()
            .is_some_and(crate::app::targets::MediaServer::claimed),
        "{server:?}"
    );
}

#[tokio::test]
async fn managed_hands_the_gate_a_plex_route_admitting_the_managed_accounts_token() {
    let http = plex(&[], None, Vec::new());
    let (ctx, at, gate) = plex_ctx("plex-managed", None, Gate::default(), http.clone());

    let state = routed(&ctx, &at).await;

    assert_eq!(state, Some(State::Wired));
    let route = gate.routes().into_iter().find(|one| one.path == SERVER);
    assert_eq!(
        route.as_ref().map(|one| &one.target),
        Some(&Target::Plex {
            pins: Vec::new(),
            link: PlexLink::Managed
        })
    );
    assert_eq!(route.as_ref().map(|one| one.credential.as_str()), Some(KEY));
    assert_eq!(route.map(|one| one.tokens), Some(vec![sha256(KEY)]));
}

#[tokio::test]
async fn owner_holds_the_account_that_claimed_plex_and_admits_no_token_of_its_own() {
    let http = plex(&[], Some(CLAIMED_BY), Vec::new());
    let (ctx, at, gate) = plex_ctx("plex-owner", Some("owner"), Gate::default(), http.clone());

    let state = routed(&ctx, &at).await;

    assert_eq!(state, Some(State::Wired));
    let route = gate.routes().into_iter().find(|one| one.path == SERVER);
    assert_eq!(
        route.as_ref().map(|one| &one.target),
        Some(&Target::Plex {
            pins: Vec::new(),
            link: PlexLink::Owner {
                account: CLAIMED_BY.to_owned()
            }
        })
    );
    assert_eq!(route.map(|one| one.tokens), Some(Vec::new()));
}

#[tokio::test]
async fn owner_before_plex_is_claimed_hands_the_gate_nothing_and_revokes_the_new_key() {
    let http = plex(&[], None, Vec::new());
    let (ctx, at, gate) = plex_ctx(
        "plex-unclaimed",
        Some("owner"),
        Gate::default(),
        http.clone(),
    );

    let state = routed(&ctx, &at).await;

    assert!(
        matches!(&state, Some(State::Skipped { reason }) if reason.contains("names no account")),
        "{state:?}"
    );
    assert!(gate.routes().is_empty());
    assert_eq!(revoked(&http), vec![KEY.to_owned()]);
}

#[tokio::test]
async fn fronted_holds_the_household_the_server_and_the_gate_tokens_it_admits() {
    let gate_token = sha256("a-gate-token");
    let held = Gate::admitting(vec![plex_route(&[sha256(KEY), gate_token.clone()])]);
    let http = plex(&[KEY], None, Vec::new());
    let (ctx, at, gate) = plex_ctx("plex-fronted", Some("fronted"), held, http);

    let state = routed(&ctx, &at).await;

    assert_eq!(state, Some(State::Wired));
    let route = gate.routes().into_iter().find(|one| one.path == SERVER);
    assert_eq!(
        route.as_ref().map(|one| &one.target),
        Some(&Target::Plex {
            pins: Vec::new(),
            link: PlexLink::Fronted {
                account: own_account(),
                members: vec![PlexMember {
                    id: "4815".to_owned(),
                    name: "Ann".to_owned()
                }],
                server: listed(),
            }
        })
    );
    assert_eq!(route.map(|one| one.tokens), Some(vec![gate_token]));
}

/// The request service at [`REQUESTS_AT`], asked over `request.intake` through `http`.
fn requests(http: &Arc<Fake>) -> intake::Adapter {
    intake::Adapter(Contracted::new(
        Arc::clone(http) as Arc<dyn crate::ports::http::Http>,
        REQUESTS_AT,
        "seerr",
        "the-requests-key",
    ))
}

/// Where `operation` of `request.intake` is asked.
fn intake_at(operation: &str) -> &'static str {
    crate::test_support::adapted::path_of(intake::CAPABILITY, intake::MAJOR, operation)
}

/// The bodies `operation` of `request.intake` was asked with over `http`.
fn asked_with(http: &Fake, operation: &str) -> Vec<serde_json::Value> {
    http.requests()
        .into_iter()
        .filter(|one| one.url.ends_with(intake_at(operation)))
        .filter_map(|one| serde_json::from_str(one.body.as_deref()?).ok())
        .collect()
}

/// Plex and a request service already set up, `establish` answered `establishing`.
fn signing(filed: &[&str], establishing: Answer) -> Arc<Fake> {
    plex(
        filed,
        None,
        vec![
            (intake_at("establish"), vec![establishing]),
            (intake_at("initialized"), vec![media::answered(&true)]),
        ],
    )
}

/// The request service signed in at the gate's Plex route over `ctx`.
async fn signed(ctx: &Ctx, project: &std::path::Path, http: &Arc<Fake>) -> State {
    let fillers = fillers(project);
    let server = crate::app::targets::MediaServer::of(&fillers);
    let Some(server) = server else {
        return State::Unmanaged;
    };
    let requests = requests(http);
    super::super::plex_linking::signed_in(ctx, &fillers, &requests, &server, true)
        .await
        .state
}

#[tokio::test]
async fn a_held_key_with_owner_unclaimed_or_a_mode_naming_none_hands_no_route() {
    let held = || Gate::admitting(vec![plex_route(&[sha256(KEY)])]);
    let (ctx, at, gate) = plex_ctx(
        "plex-held-unclaimed",
        Some("owner"),
        held(),
        plex(&[KEY], None, Vec::new()),
    );
    let unclaimed = routed(&ctx, &at).await;
    let (ctx, at, _) = plex_ctx(
        "plex-held-unnamed",
        Some("shared"),
        held(),
        plex(&[KEY], None, Vec::new()),
    );
    let unnamed = routed(&ctx, &at).await;

    assert!(
        matches!(unclaimed, Some(State::Skipped { .. })),
        "{unclaimed:?}"
    );
    assert_eq!(gate.routes(), vec![plex_route(&[sha256(KEY)])]);
    assert!(matches!(unnamed, Some(State::Failed { .. })), "{unnamed:?}");
}

mod fronting;
mod linking;
mod signing;
