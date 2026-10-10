//! The request service pointed at the media server through the request gate, and handed the
//! token it reaches the media server with after its setup.

use lemonfiber_sidecar::gate::{Accepted, File, Tokens, PORT};
use lemonfiber_sidecar::TokenHash;

use super::*;

/// What the fixed randomness renders to, which is every token minted here.
fn minted() -> String {
    crate::secret::render(&[0xab; crate::secret::SECRET_BYTES])
}

/// A stack with the media server, the request service and, where `gating`, the gate.
fn stack(gating: bool) -> Vec<lemonfiber_manifest::Service> {
    let mut services = vec![media_server_svc(), requests_svc()];
    if gating {
        services.push(manifest_service("request-gate", None, Some(PORT)));
    }
    services
}

/// Where the gate reads its tokens, under `project`.
fn tokens_file(project: &std::path::Path) -> std::path::PathBuf {
    crate::app::gating::path(project, File::Tokens)
}

/// The gate accepting `tokens` on the media server's route.
fn accepting(tokens: &[&str]) -> Tokens {
    Tokens::of(vec![Accepted {
        route: "jellyfin".to_owned(),
        tokens: tokens.iter().map(|token| TokenHash::of(token)).collect(),
    }])
}

/// A stack directory where the gate accepts `accepted` on the media server's route.
fn project(name: &str, accepted: &[&str]) -> std::path::PathBuf {
    let at = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(&at);
    if !accepted.is_empty() {
        let _ = store::write(&tokens_file(&at), &accepting(accepted).written());
    }
    at
}

/// What the gate accepts under `project`, where its file reads.
fn accepted(project: &std::path::Path) -> Option<Tokens> {
    std::fs::read_to_string(tokens_file(project))
        .ok()
        .and_then(|text| Tokens::read(&text).ok())
}

/// The request service's media-server settings: at `host`, `port` and `base`, with
/// `key`.
fn link(host: &str, port: u16, base: &str, key: &str) -> String {
    serde_json::json!({
        "name": "Jellyfin",
        "ip": host,
        "port": port,
        "useSsl": false,
        "urlBase": base,
        "apiKey": key,
    })
    .to_string()
}

/// Linked at the gate with `key`.
fn at_the_gate(key: &str) -> String {
    link("request-gate", PORT, "/jellyfin", key)
}

/// A household whose media server has been set up, whose request service answers each of
/// `initialised` in turn when asked whether it is set up, lists `linked` as its
/// media-server settings, and answers a new link with `relinked`.
fn serving(initialised: &[bool], linked: &str, relinked: u16) -> Arc<Fake> {
    Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/System/Info/Public",
            vec![Answer::reply(200, r#"{"StartupWizardCompleted":true}"#)],
        ),
        (Method::Post, "/auth/jellyfin", vec![Answer::reply(200, "")]),
        (
            Method::Post,
            "/settings/initialize",
            vec![Answer::reply(200, "")],
        ),
        (
            Method::Get,
            "/settings/public",
            initialised
                .iter()
                .map(|done| Answer::reply(200, format!(r#"{{"initialized":{done}}}"#)))
                .collect(),
        ),
        (
            Method::Get,
            "/settings/jellyfin",
            vec![Answer::reply(200, linked.to_owned())],
        ),
        (
            Method::Post,
            "/settings/jellyfin",
            vec![Answer::reply(relinked, "")],
        ),
        (
            Method::Post,
            "/Users/AuthenticateByName",
            vec![Answer::reply(
                200,
                r#"{"AccessToken":"token","User":{"Id":"admin-id"}}"#,
            )],
        ),
        (
            Method::Post,
            "/Users/admin-id/Password",
            vec![Answer::reply(204, "")],
        ),
        (
            Method::Get,
            "/settings/notifications/webpush",
            vec![Answer::reply(200, r#"{"enabled":true,"types":222}"#)],
        ),
        (
            Method::Post,
            "/settings/notifications/webpush",
            vec![Answer::reply(200, "")],
        ),
    ])
}

/// Both halves of the identity step over `http`, on `services` under `project`.
async fn seeded(
    name: &str,
    services: &[lemonfiber_manifest::Service],
    project: &std::path::Path,
    http: &Arc<Fake>,
    random: bool,
    rehearsing: bool,
) -> Vec<Wiring> {
    let bytes = random.then(|| vec![0xab; crate::secret::SECRET_BYTES]);
    let mut ctx =
        seed_ctx(None, true, Vec::new(), bytes, Some(recorded_admin(name))).with_http(http.clone());
    ctx.dry_run = rehearsing;
    seeded_with(&ctx, services, project).await
}

/// Both halves of the identity step in `ctx`.
async fn seeded_with(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    project: &std::path::Path,
) -> Vec<Wiring> {
    let server = served(services);
    let admin = super::super::identity::seed_media_server_admin(ctx, server.as_ref()).await;
    super::super::seed_request_identity(
        ctx,
        services,
        &crate::baseline::Baseline::new(),
        server.as_ref(),
        admin,
        Some(project),
    )
    .await
    .0
}

/// The state of the connection called `name`, where the step reports one.
fn state_of(wirings: &[Wiring], name: &str) -> Option<State> {
    wirings
        .iter()
        .find(|wiring| wiring.connection == name)
        .map(|wiring| wiring.state.clone())
}

/// What the report calls the link.
const LINK: &str = "Seerr's Jellyfin connection";

/// A fresh request service is set up at the gate's media server route, then handed a
/// token the gate accepts there, which it proves before it keeps it.
#[tokio::test]
async fn a_fresh_request_service_is_set_up_through_the_gate() {
    let at = project("linking-fresh", &[]);
    let http = serving(&[false, true], &at_the_gate("a-placeholder"), 200);

    let wirings = seeded("linking-fresh", &stack(true), &at, &http, true, false).await;

    assert_eq!(
        state_of(&wirings, crate::seed::IDENTITY),
        Some(State::Wired)
    );
    assert_eq!(state_of(&wirings, LINK), Some(State::Wired));
    let sent = |method: Method, path: &str| {
        http.requests()
            .into_iter()
            .find(|asked| asked.method == method && asked.url.contains(path))
            .and_then(|asked| asked.body)
            .unwrap_or_default()
    };
    let signed_in = sent(Method::Post, "/auth/jellyfin");
    assert!(
        signed_in.contains("\"hostname\":\"request-gate\"")
            && signed_in.contains(&format!("\"port\":{PORT}"))
            && signed_in.contains("\"urlBase\":\"/jellyfin\""),
        "{signed_in}"
    );
    let linked = sent(Method::Post, "/settings/jellyfin");
    assert!(
        linked.contains(&format!("\"apiKey\":\"{}\"", minted()))
            && linked.contains("\"ip\":\"request-gate\""),
        "{linked}"
    );
    assert_eq!(accepted(&at), Some(accepting(&[&minted()])));
}

/// A token the request service holds at the gate, which the gate accepts, is left.
#[tokio::test]
async fn a_link_both_sides_hold_is_left_alone() {
    let at = project("linking-held", &["held"]);
    let http = serving(&[true], &at_the_gate("held"), 200);

    let wirings = seeded("linking-held", &stack(true), &at, &http, true, false).await;

    assert_eq!(state_of(&wirings, LINK), Some(State::AlreadyWired));
    assert_eq!(accepted(&at), Some(accepting(&["held"])));
}

/// A request service set up at the media server's own address is moved to the gate.
#[tokio::test]
async fn a_request_service_linked_directly_is_moved_to_the_gate() {
    let at = project("linking-direct", &[]);
    let http = serving(&[true], &link("jellyfin", 8096, "", "seerrs-own"), 200);

    let wirings = seeded("linking-direct", &stack(true), &at, &http, true, false).await;

    assert_eq!(state_of(&wirings, LINK), Some(State::Wired));
    assert_eq!(accepted(&at), Some(accepting(&[&minted()])));
}

/// A stack without the gate has no link to hold, and sets the request service up at
/// the media server's own address.
#[tokio::test]
async fn without_the_gate_there_is_no_link() {
    let at = project("linking-ungated", &[]);
    let http = serving(&[false, true], &at_the_gate("x"), 200);

    let wirings = seeded("linking-ungated", &stack(false), &at, &http, true, false).await;

    assert_eq!(state_of(&wirings, LINK), None);
    assert!(http.requests().iter().any(|asked| asked
        .body
        .as_deref()
        .is_some_and(|body| body.contains("\"hostname\":\"jellyfin\""))));
}

/// A rehearsal says where the request service would be pointed, and writes nothing.
#[tokio::test]
async fn a_rehearsal_says_where_and_writes_nothing() {
    let gate = Some("at the request gate".to_owned());
    for (name, initialised, linked, expected) in [
        (
            "linking-would-set-up",
            false,
            at_the_gate(""),
            State::WouldWire {
                yours: None,
                ours: gate.clone(),
            },
        ),
        (
            "linking-would-move",
            true,
            link("jellyfin", 8096, "", "seerrs-own"),
            State::WouldWire {
                yours: Some("at jellyfin:8096".to_owned()),
                ours: gate.clone(),
            },
        ),
        (
            "linking-would-replace",
            true,
            at_the_gate("unaccepted"),
            State::WouldWire {
                yours: None,
                ours: gate.clone(),
            },
        ),
        (
            "linking-would-name",
            true,
            link("", 0, "", ""),
            State::WouldWire {
                yours: None,
                ours: gate.clone(),
            },
        ),
    ] {
        let at = project(name, &[]);
        let http = serving(&[initialised], &linked, 200);

        let wirings = seeded(name, &stack(true), &at, &http, true, true).await;

        assert_eq!(state_of(&wirings, LINK), Some(expected), "{name}");
        assert!(
            !http
                .requests()
                .iter()
                .any(|asked| asked.url.contains("/settings/jellyfin")
                    && asked.method == Method::Post),
            "{name}"
        );
        assert_eq!(accepted(&at), None, "{name}");
    }
}

/// A request service that will not take the token says so, and the gate keeps
/// accepting what it did.
#[tokio::test]
async fn a_token_the_request_service_refuses_is_said() {
    let at = project("linking-refused", &["older"]);
    let http = serving(&[true], &at_the_gate("stale"), 400);

    let wirings = seeded("linking-refused", &stack(true), &at, &http, true, false).await;

    let detail = match state_of(&wirings, LINK) {
        Some(State::Failed { detail }) => detail,
        other => format!("{other:?}"),
    };
    assert!(
        detail.starts_with("Seerr did not take the gate's Jellyfin token: "),
        "{detail}"
    );
    assert_eq!(accepted(&at), Some(accepting(&["older", &minted()])));
}

/// A link that cannot be read, a token that cannot be minted, and tokens the gate
/// cannot be handed each fail the link rather than call it done.
#[tokio::test]
async fn a_link_that_cannot_be_made_fails() {
    let at = project("linking-unread", &[]);
    let http = serving(&[true], "not settings", 200);
    let wirings = seeded("linking-unread", &stack(true), &at, &http, true, false).await;
    assert!(
        matches!(state_of(&wirings, LINK), Some(State::Failed { .. })),
        "{wirings:?}"
    );

    let at = project("linking-unrandom", &[]);
    let http = serving(&[true], &at_the_gate("stale"), 200);
    let wirings = seeded("linking-unrandom", &stack(true), &at, &http, false, false).await;
    assert_eq!(
        state_of(&wirings, LINK),
        Some(State::Failed {
            detail: "no randomness was available to generate a token".to_owned(),
        })
    );

    let at = project("linking-unwritable", &[]);
    let _ = std::fs::create_dir_all(tokens_file(&at).join("blocked"));
    let http = serving(&[true], &at_the_gate("stale"), 200);
    let wirings = seeded("linking-unwritable", &stack(true), &at, &http, true, false).await;
    let detail = match state_of(&wirings, LINK) {
        Some(State::Failed { detail }) => detail,
        other => format!("{other:?}"),
    };
    assert!(
        detail.starts_with(&format!(
            "the tokens could not be written to {}",
            tokens_file(&at).display()
        )),
        "{detail}"
    );
    assert!(!http
        .requests()
        .iter()
        .any(|asked| asked.method == Method::Post && asked.url.contains("/settings/jellyfin")));
}

/// An old token the gate cannot be told to stop accepting is said.
#[tokio::test]
async fn an_old_token_that_cannot_be_retired_is_said() {
    let at = project("linking-unretired", &[]);
    let _ = std::fs::create_dir_all(tokens_file(&at).join("blocked"));
    let both = accepting(&["older", &minted()]).written();
    let http = serving(&[true], &at_the_gate("stale"), 200);
    let ctx = seed_ctx(
        None,
        true,
        Vec::new(),
        Some(vec![0xab; crate::secret::SECRET_BYTES]),
        Some(recorded_admin("linking-unretired")),
    )
    .with_http(http.clone())
    .with_filesystem(lemonfiber_fixtures::files::Files::at(vec![(
        tokens_file(&at),
        &both,
    )]));

    let wirings = seeded_with(&ctx, &stack(true), &at).await;

    let detail = match state_of(&wirings, LINK) {
        Some(State::Failed { detail }) => detail,
        other => format!("{other:?}"),
    };
    assert!(
        detail.starts_with("the tokens could not be written to "),
        "{detail}"
    );
}

/// Where the request service could not be set up, it is not handed a link either.
#[tokio::test]
async fn a_request_service_not_set_up_is_not_linked() {
    let at = project("linking-unset", &[]);
    let http = serving(&[false], &at_the_gate("x"), 200);

    let wirings = seeded("linking-unset", &stack(true), &at, &http, true, false).await;

    assert!(
        matches!(
            state_of(&wirings, crate::seed::IDENTITY),
            Some(State::Failed { .. })
        ),
        "{wirings:?}"
    );
    assert_eq!(state_of(&wirings, LINK), None);
}
