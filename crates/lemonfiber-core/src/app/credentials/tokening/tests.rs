use std::path::{Path, PathBuf};
use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_sidecar::gate::{
    Accepted, Credential, File, Kind, Tokens, Upstream, Upstreams, PORT,
};
use lemonfiber_sidecar::TokenHash;

use super::{held, rotate, UNPRINTED};
use crate::app::Ctx;
use crate::config::{store, Settings};
use crate::credential::{Held, Reach, Settled, State};
use crate::ports::service::{Endpoint, MediaServerLink, RegisteredTarget};
use crate::test_support::a_context;
use crate::test_support::curators::TELEVISION;
use crate::test_support::requests::{
    answered, intake, refusing, taken, told, unavailable, REQUESTS,
};

/// The key lemonfiber minted for the curator, which the gate presents on its route.
const CURATOR_KEY: &str = "sonarrs";

/// What the fixed randomness renders to, which is every token minted here.
fn minted() -> String {
    crate::secret::render(&[0xab; crate::secret::SECRET_BYTES])
}

/// A service in the stack, with the shape this module reads.
fn service(id: &str, name: &str, port: u16) -> lemonfiber_manifest::Service {
    lemonfiber_manifest::Service {
        id: id.to_owned(),
        name: name.to_owned(),
        profile: "media".to_owned(),
        image: "example/image".to_owned(),
        tag: "1".to_owned(),
        digest: None,
        port: Some(port),
        bind: None,
        health: None,
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
        certificate: None,
        asks_for: None,
        reaches: None,
        listens: None,
        shares_config: None,
        rebuilt: Vec::new(),
    }
}

/// The stack's fillers with `stack(gating)` as its services, written at `project`.
fn fillers(gating: bool, project: Option<&std::path::Path>) -> crate::wiring::Fillers {
    fillers_from(stack(gating), project)
}

/// The stack's fillers with `services` as its services, written at `project`, the request
/// service moved into its plugin.
fn fillers_from(
    services: Vec<lemonfiber_manifest::Service>,
    project: Option<&std::path::Path>,
) -> crate::wiring::Fillers {
    let moved: Vec<_> = REQUESTS.installed().into_iter().collect();
    fillers_beside(services, &moved, project)
}

/// The same, with `installed` beside the stack and the media server and the curator moved
/// into their plugins.
fn fillers_beside(
    services: Vec<lemonfiber_manifest::Service>,
    installed: &[crate::plugin::Installed],
    project: Option<&std::path::Path>,
) -> crate::wiring::Fillers {
    let installed: Vec<_> = crate::test_support::media::moved()
        .into_iter()
        .chain(TELEVISION.installed())
        .chain(installed.iter().cloned())
        .collect();
    crate::test_support::stack_fillers(
        services,
        &installed,
        project,
        crate::plugin::first_party::EMBEDDED,
    )
}

/// The media server, the request service, a curator and, where `gating`, the request gate.
fn stack(gating: bool) -> Vec<lemonfiber_manifest::Service> {
    let mut media_server = service("jellyfin", "Jellyfin", 8096);
    media_server.provides = vec!["media.serve".to_owned(), "identity.source".to_owned()];
    media_server.listens = Some(8096);
    let mut requests = service("seerr", "Seerr", 5055);
    requests.provides = vec!["request.intake".to_owned()];
    let mut curator = service("sonarr", "Sonarr", 8989);
    curator.media_types = vec!["tv".to_owned()];
    curator.listens = Some(8989);
    curator.provides = vec!["library.curate".to_owned()];
    let mut services = vec![media_server, requests, curator];
    if gating {
        let mut gate = service("request-gate", "Request gate", PORT);
        gate.provides = vec!["request.gate".to_owned()];
        services.push(gate);
    }
    services
}

/// The gate's routes: the curator's and the media server's.
fn routes() -> Upstreams {
    Upstreams::of(vec![
        Upstream {
            route: "sonarr".to_owned(),
            kind: Kind::Sonarr,
            address: "http://sonarr:8989".to_owned(),
            credential: Credential::new(CURATOR_KEY),
            majors: Vec::new(),
            pins: lemonfiber_sidecar::Pins::default(),
        },
        Upstream {
            route: "jellyfin".to_owned(),
            kind: Kind::Jellyfin,
            address: "http://jellyfin:8096".to_owned(),
            credential: Credential::new("gates"),
            majors: vec![10],
            pins: lemonfiber_sidecar::Pins::default(),
        },
    ])
}

/// The gate accepting `curator` on the curator's route and `media_server` on the media
/// server's.
fn accepting(curator: &[&str], media_server: &[&str]) -> Tokens {
    let hashed = |tokens: &[&str]| tokens.iter().map(|token| TokenHash::of(token)).collect();
    Tokens::of(vec![
        Accepted {
            route: "sonarr".to_owned(),
            tokens: hashed(curator),
        },
        Accepted {
            route: "jellyfin".to_owned(),
            tokens: hashed(media_server),
        },
    ])
}

fn tokens_file(project: &Path) -> PathBuf {
    crate::app::gating::path(project, File::Tokens)
}

/// A stack directory whose gate answers its routes, accepting `accepted`, and a context
/// over `http` with randomness where `random`.
fn scene(
    name: &str,
    routed: bool,
    accepted: &Tokens,
    http: Arc<Fake>,
    random: bool,
) -> (Ctx, PathBuf) {
    let at = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(&at);
    if routed {
        let _ = store::write(
            &crate::app::gating::path(&at, File::Upstreams),
            &routes().written(),
        );
    }
    let _ = store::write(&tokens_file(&at), &accepted.written());
    let env = at.join(".env");
    let _ = store::set(
        &env,
        crate::config::IDENTITY_SOURCE_ADMIN_PASSWORD_KEY,
        "minted-earlier",
    );
    for adapter in [crate::test_support::media::ADAPTER, REQUESTS.adapter] {
        let keyed = crate::plugin::key_file(&at, adapter);
        let _ = std::fs::create_dir_all(keyed.parent().unwrap_or(&at));
        let _ = std::fs::write(&keyed, crate::test_support::media::KEY);
    }
    let ctx = a_context()
        .engine(Arc::new(
            crate::test_support::media::running().alongside(REQUESTS.running()),
        ))
        .settings(Settings {
            env_file: Some(env),
            ..Settings::default()
        })
        .build()
        .with_http(http)
        .with_random(Arc::new(lemonfiber_fixtures::support::FixedRandom(
            random.then(|| vec![0xab; crate::secret::SECRET_BYTES]),
        )));
    TELEVISION.minted(&ctx, CURATOR_KEY);
    (ctx, at)
}

/// What the gate accepts under `project`.
fn accepted(project: &Path) -> Option<Tokens> {
    std::fs::read_to_string(tokens_file(project))
        .ok()
        .and_then(|text| Tokens::read(&text).ok())
}

/// The request service: the curator held at the gate with `curator`, the media server
/// linked at the gate with `media_server`; answering a move with `moved`, its own test with
/// each of `tested` in turn, and a new link with `linked`.
fn serving(
    curator: &str,
    media_server: &str,
    moved: Answer,
    tested: Vec<Answer>,
    linked: Answer,
) -> Arc<Fake> {
    let through = |base: &str| Endpoint {
        host: crate::app::gating::SERVICE.to_owned(),
        port: PORT,
        base: base.to_owned(),
    };
    beside_the_gates_key(vec![
        (
            intake("fulfilment_targets"),
            vec![answered(&[RegisteredTarget {
                id: "1".to_owned(),
                at: through("/sonarr"),
                key: curator.to_owned(),
                kind: crate::ports::media::Kind::Tv,
            }])],
        ),
        (
            intake("media_server_link"),
            vec![answered(&MediaServerLink {
                at: through("/jellyfin"),
                key: media_server.to_owned(),
            })],
        ),
        (intake("move_fulfilment_target"), vec![moved.clone(), moved]),
        (intake("test_fulfilment_target"), tested),
        (intake("link_media_server"), vec![linked]),
    ])
}

/// [`serving`], taking every write and proving every target.
fn taking(curator: &str, media_server: &str) -> Arc<Fake> {
    serving(curator, media_server, taken(), vec![taken()], taken())
}

/// A household answering the media server's keys filed under the gate's name, through
/// which a rotation finds the key the gate presents on its route there, and then `routes`.
fn beside_the_gates_key(routes: Vec<(&'static str, Vec<Answer>)>) -> Arc<Fake> {
    let mut answering = vec![(
        crate::test_support::media::serve("filed_as"),
        vec![Answer::reply(200, crate::test_support::json(&["gates"]))],
    )];
    answering.extend(routes);
    Fake::by_path_in_turn(answering)
}

/// Whether the request service was asked to change anything.
fn written(http: &Fake) -> bool {
    http.requests().iter().any(|asked| {
        asked.url.ends_with(intake("move_fulfilment_target"))
            || asked.url.ends_with(intake("link_media_server"))
    })
}

/// The line named `name`, or an empty one the assertions after it then fail on.
fn named(lines: &[Held], name: &str) -> Held {
    lines
        .iter()
        .find(|line| line.name == name)
        .cloned()
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

fn unproven(settled: &Settled) -> Option<&str> {
    match settled {
        Settled::Unproven { detail } => Some(detail),
        _ => None,
    }
}

const CURATOR_LINE: &str = "Request gate token for Sonarr";
const MEDIA_SERVER_LINE: &str = "Request gate token for Jellyfin";

#[tokio::test]
async fn each_route_has_a_line_read_from_the_request_service() {
    let http = taking("held", "linked");
    let (ctx, at) = scene(
        "tokens-listed",
        true,
        &accepting(&["held"], &["other"]),
        http,
        true,
    );

    let lines = held(&ctx, &stack(true), &fillers(true, Some(&at)), Some(&at)).await;

    let curator = named(&lines, CURATOR_LINE);
    assert_eq!(curator.state, State::Active);
    assert_eq!(
        curator.fingerprint,
        Some(crate::credential::fingerprint("held"))
    );
    assert_eq!(curator.location, "held only by the request service");
    assert_eq!(
        curator.consumers,
        vec!["the request service, which reaches Sonarr through the gate".to_owned()]
    );
    let media_server = named(&lines, MEDIA_SERVER_LINE);
    assert_eq!(media_server.state, State::Invalid);
    assert!(media_server
        .advisory
        .is_some_and(|said| said.starts_with("the gate does not accept the token")));
}

#[tokio::test]
async fn a_token_not_handed_over_or_unread_says_so() {
    let http = Fake::by_path(vec![
        (
            intake("fulfilment_targets"),
            answered(&Vec::<RegisteredTarget>::new()),
        ),
        (
            intake("media_server_link"),
            answered(&MediaServerLink {
                at: Endpoint {
                    host: "jellyfin".to_owned(),
                    port: 8096,
                    base: String::new(),
                },
                key: "own".to_owned(),
            }),
        ),
    ]);
    let (ctx, at) = scene("tokens-unheld", true, &accepting(&[], &[]), http, true);
    let lines = held(&ctx, &stack(true), &fillers(true, Some(&at)), Some(&at)).await;
    for name in [CURATOR_LINE, MEDIA_SERVER_LINE] {
        let line = named(&lines, name);
        assert_eq!(line.state, State::Absent, "{name}");
        assert!(line
            .advisory
            .is_some_and(|said| said.ends_with("Run lemonfiber seed.")));
    }

    let silent = Fake::by_path(vec![(intake(""), unavailable())]);
    let (ctx, at) = scene("tokens-unread", true, &accepting(&[], &[]), silent, true);
    let lines = held(&ctx, &stack(true), &fillers(true, Some(&at)), Some(&at)).await;
    assert_eq!(named(&lines, CURATOR_LINE).state, State::Stale);
    assert_eq!(named(&lines, MEDIA_SERVER_LINE).state, State::Stale);
}

#[tokio::test]
async fn without_the_gate_its_routes_or_a_stack_directory_there_are_no_lines() {
    let http = taking("held", "linked");
    let (ctx, at) = scene(
        "tokens-none",
        true,
        &accepting(&[], &[]),
        http.clone(),
        true,
    );
    assert!(
        held(&ctx, &stack(false), &fillers(false, Some(&at)), Some(&at))
            .await
            .is_empty()
    );
    assert!(held(&ctx, &stack(true), &fillers(true, None), None)
        .await
        .is_empty());

    let (ctx, at) = scene("tokens-unrouted", false, &accepting(&[], &[]), http, true);
    assert!(
        held(&ctx, &stack(true), &fillers(true, Some(&at)), Some(&at))
            .await
            .is_empty()
    );
}

/// A gate the stack's own credentials may not cross to, another plugin's, has no lines:
/// nothing is read of what it admits.
#[tokio::test]
async fn a_gate_another_plugin_brings_has_no_lines() {
    let (ctx, at) = scene(
        "tokens-strangers-gate",
        true,
        &accepting(&["held"], &[]),
        taking("held", "linked"),
        true,
    );
    let installed: Vec<_> = REQUESTS
        .installed()
        .into_iter()
        .chain([crate::test_support::contracted(
            "gatekeeper",
            "gatekeeper",
            "request.gate",
        )])
        .collect();
    let fillers = fillers_beside(stack(false), &installed, Some(&at));

    assert!(held(&ctx, &stack(true), &fillers, Some(&at))
        .await
        .is_empty());
}

#[tokio::test]
async fn a_token_is_never_printed() {
    let http = taking("held", "linked");
    let (ctx, at) = scene(
        "tokens-unprinted",
        true,
        &accepting(&["held"], &[]),
        http,
        true,
    );
    let line = named(
        &held(&ctx, &stack(true), &fillers(true, Some(&at)), Some(&at)).await,
        CURATOR_LINE,
    );

    let reaching = fillers(true, Some(&at));
    let shown = super::super::revealing::reveal(&ctx, &line, true, (&reaching, Some(&at))).await;

    assert_eq!(shown.value, None);
    assert_eq!(shown.warning, UNPRINTED);
}

#[tokio::test]
async fn a_route_for_a_service_the_stack_no_longer_names_is_called_by_its_route() {
    let http = taking("held", "linked");
    let (ctx, at) = scene(
        "tokens-unnamed",
        true,
        &accepting(&["held"], &[]),
        http,
        true,
    );
    let without_curator: Vec<_> = stack(true)
        .into_iter()
        .filter(|service| service.id != "sonarr")
        .collect();

    let lines = held(
        &ctx,
        &without_curator,
        &fillers_from(without_curator.clone(), Some(&at)),
        Some(&at),
    )
    .await;

    assert_eq!(
        named(&lines, "Request gate token for sonarr").state,
        State::Active
    );
}

mod rotating;
