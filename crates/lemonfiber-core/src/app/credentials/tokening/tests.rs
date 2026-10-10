use std::path::{Path, PathBuf};
use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_ports::http::Method;
use lemonfiber_sidecar::gate::{
    Accepted, Credential, File, Kind, Tokens, Upstream, Upstreams, PORT,
};
use lemonfiber_sidecar::TokenHash;

use super::{held, rotate, UNPRINTED};
use crate::app::Ctx;
use crate::config::{store, Settings};
use crate::credential::{Held, Reach, Settled, State};
use crate::test_support::a_context;

/// What the fixed randomness renders to, which is every token minted here.
fn minted() -> String {
    crate::secret::render(&[0xab; crate::secret::SECRET_BYTES])
}

/// A service in the stack, with the shape this module reads.
fn service(
    id: &str,
    name: &str,
    api: Option<lemonfiber_manifest::ApiKind>,
    port: u16,
) -> lemonfiber_manifest::Service {
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
        listens: None,
    }
}

/// The stack's fillers with `stack(gating)` as its services, written at `project`.
fn fillers(gating: bool, project: Option<&std::path::Path>) -> crate::wiring::Fillers {
    fillers_from(stack(gating), project)
}

/// The stack's fillers with `services` as its services, written at `project`.
fn fillers_from(
    services: Vec<lemonfiber_manifest::Service>,
    project: Option<&std::path::Path>,
) -> crate::wiring::Fillers {
    fillers_beside(services, &[], project)
}

/// The same, with `installed` beside the stack.
fn fillers_beside(
    services: Vec<lemonfiber_manifest::Service>,
    installed: &[crate::plugin::Installed],
    project: Option<&std::path::Path>,
) -> crate::wiring::Fillers {
    crate::test_support::stack_fillers(
        services,
        installed,
        project,
        crate::plugin::first_party::EMBEDDED,
    )
}

/// Jellyfin, the request service, Sonarr and, where `gating`, the request gate.
fn stack(gating: bool) -> Vec<lemonfiber_manifest::Service> {
    let mut jellyfin = service(
        "jellyfin",
        "Jellyfin",
        Some(lemonfiber_manifest::ApiKind::Jellyfin),
        8096,
    );
    jellyfin.provides = vec!["media.serve".to_owned(), "identity.source".to_owned()];
    let mut requests = service(
        "seerr",
        "Seerr",
        Some(lemonfiber_manifest::ApiKind::Seerr),
        5055,
    );
    requests.provides = vec!["request.intake".to_owned()];
    let mut services = vec![jellyfin, requests, service("sonarr", "Sonarr", None, 8989)];
    if gating {
        services.push(service("request-gate", "Request gate", None, PORT));
    }
    services
}

/// The gate's routes: Sonarr's and Jellyfin's.
fn routes() -> Upstreams {
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
            credential: Credential::new("gates"),
            majors: vec![10],
        },
    ])
}

/// The gate accepting `sonarr` on Sonarr's route and `jellyfin` on Jellyfin's.
fn accepting(sonarr: &[&str], jellyfin: &[&str]) -> Tokens {
    let hashed = |tokens: &[&str]| tokens.iter().map(|token| TokenHash::of(token)).collect();
    Tokens::of(vec![
        Accepted {
            route: "sonarr".to_owned(),
            tokens: hashed(sonarr),
        },
        Accepted {
            route: "jellyfin".to_owned(),
            tokens: hashed(jellyfin),
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
    let ctx = a_context()
        .settings(Settings::default())
        .build()
        .with_http(http)
        .with_random(Arc::new(lemonfiber_fixtures::support::FixedRandom(
            random.then(|| vec![0xab; crate::secret::SECRET_BYTES]),
        )));
    (ctx, at)
}

/// What the gate accepts under `project`.
fn accepted(project: &Path) -> Option<Tokens> {
    std::fs::read_to_string(tokens_file(project))
        .ok()
        .and_then(|text| Tokens::read(&text).ok())
}

/// The request service: Sonarr held at the gate with `sonarr`, Jellyfin linked at the
/// gate with `jellyfin`; answering a move with `moved`, its own test with each of
/// `tested` in turn, and a new link with `linked`.
fn serving(sonarr: &str, jellyfin: &str, moved: u16, tested: &[u16], linked: u16) -> Arc<Fake> {
    let target = serde_json::json!([{
        "id": 1, "hostname": "request-gate", "port": PORT, "baseUrl": "/sonarr", "apiKey": sonarr,
    }])
    .to_string();
    let link = serde_json::json!({
        "ip": "request-gate", "port": PORT, "urlBase": "/jellyfin", "apiKey": jellyfin,
    })
    .to_string();
    Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/settings/radarr",
            vec![Answer::reply(200, "[]")],
        ),
        (
            Method::Get,
            "/settings/sonarr",
            vec![Answer::reply(200, target)],
        ),
        (
            Method::Get,
            "/settings/jellyfin",
            vec![Answer::reply(200, link)],
        ),
        (
            Method::Put,
            "/settings/sonarr/1",
            vec![Answer::reply(moved, "")],
        ),
        (
            Method::Post,
            "/settings/sonarr/test",
            tested
                .iter()
                .map(|status| Answer::reply(*status, ""))
                .collect(),
        ),
        (
            Method::Post,
            "/settings/jellyfin",
            vec![Answer::reply(linked, "")],
        ),
    ])
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

const SONARR: &str = "Request gate token for Sonarr";
const JELLYFIN: &str = "Request gate token for Jellyfin";

#[tokio::test]
async fn each_route_has_a_line_read_from_the_request_service() {
    let http = serving("held", "linked", 200, &[200], 200);
    let (ctx, at) = scene(
        "tokens-listed",
        true,
        &accepting(&["held"], &["other"]),
        http,
        true,
    );

    let lines = held(&ctx, &stack(true), &fillers(true, Some(&at)), Some(&at)).await;

    let sonarr = named(&lines, SONARR);
    assert_eq!(sonarr.state, State::Active);
    assert_eq!(
        sonarr.fingerprint,
        Some(crate::credential::fingerprint("held"))
    );
    assert_eq!(sonarr.location, "held only by the request service");
    assert_eq!(
        sonarr.consumers,
        vec!["the request service, which reaches Sonarr through the gate".to_owned()]
    );
    let jellyfin = named(&lines, JELLYFIN);
    assert_eq!(jellyfin.state, State::Invalid);
    assert!(jellyfin
        .advisory
        .is_some_and(|said| said.starts_with("the gate does not accept the token")));
}

#[tokio::test]
async fn a_token_not_handed_over_or_unread_says_so() {
    let http = Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/settings/radarr",
            vec![Answer::reply(200, "[]")],
        ),
        (
            Method::Get,
            "/settings/sonarr",
            vec![Answer::reply(200, "[]")],
        ),
        (
            Method::Get,
            "/settings/jellyfin",
            vec![Answer::reply(
                200,
                r#"{"ip":"jellyfin","port":8096,"apiKey":"own"}"#,
            )],
        ),
    ]);
    let (ctx, at) = scene("tokens-unheld", true, &accepting(&[], &[]), http, true);
    let lines = held(&ctx, &stack(true), &fillers(true, Some(&at)), Some(&at)).await;
    for name in [SONARR, JELLYFIN] {
        let line = named(&lines, name);
        assert_eq!(line.state, State::Absent, "{name}");
        assert!(line
            .advisory
            .is_some_and(|said| said.ends_with("Run `lemonfiber seed`.")));
    }

    let silent = Fake::by_route_in_turn(vec![(Method::Get, "", vec![Answer::Silent])]);
    let (ctx, at) = scene("tokens-unread", true, &accepting(&[], &[]), silent, true);
    let lines = held(&ctx, &stack(true), &fillers(true, Some(&at)), Some(&at)).await;
    assert_eq!(named(&lines, SONARR).state, State::Stale);
    assert_eq!(named(&lines, JELLYFIN).state, State::Stale);
}

#[tokio::test]
async fn without_the_gate_its_routes_or_a_stack_directory_there_are_no_lines() {
    let http = serving("held", "linked", 200, &[200], 200);
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

#[tokio::test]
async fn a_token_is_never_printed() {
    let http = serving("held", "linked", 200, &[200], 200);
    let (ctx, at) = scene(
        "tokens-unprinted",
        true,
        &accepting(&["held"], &[]),
        http,
        true,
    );
    let line = named(
        &held(&ctx, &stack(true), &fillers(true, Some(&at)), Some(&at)).await,
        SONARR,
    );

    let shown = super::super::revealing::reveal(&ctx, &line, true).await;

    assert_eq!(shown.value, None);
    assert_eq!(shown.warning, UNPRINTED);
}

#[tokio::test]
async fn a_route_for_a_service_the_stack_no_longer_names_is_called_by_its_route() {
    let http = serving("held", "linked", 200, &[200], 200);
    let (ctx, at) = scene(
        "tokens-unnamed",
        true,
        &accepting(&["held"], &[]),
        http,
        true,
    );
    let without_sonarr: Vec<_> = stack(true)
        .into_iter()
        .filter(|service| service.id != "sonarr")
        .collect();

    let lines = held(
        &ctx,
        &without_sonarr,
        &fillers_from(without_sonarr.clone(), Some(&at)),
        Some(&at),
    )
    .await;

    assert_eq!(
        named(&lines, "Request gate token for sonarr").state,
        State::Active
    );
}

mod rotating;
