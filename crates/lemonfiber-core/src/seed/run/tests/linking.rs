//! The request service pointed at the media server through the request gate, and handed the
//! token it reaches the media server with after its setup.

use lemonfiber_sidecar::gate::{Accepted, File, Kind, Tokens, PORT};
use lemonfiber_sidecar::TokenHash;

use super::*;
use crate::ports::service::{Endpoint, IdentitySource, MediaServerLink};
use crate::test_support::{media, requests};

/// What the fixed randomness renders to, which is every token minted here.
fn minted() -> String {
    crate::secret::render(&[0xab; crate::secret::SECRET_BYTES])
}

/// A stack with the media server, the request service and, where `gating`, the gate.
fn stack(gating: bool) -> Vec<lemonfiber_manifest::Service> {
    let mut services = vec![media_server_svc(), requests_svc()];
    if gating {
        services.push(request_gate());
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

/// The key the gate presents on its media server route, filed under its name there.
const GATES_KEY: &str = "gates-key";

/// A stack directory where the gate routes to the media server under [`GATES_KEY`] and
/// accepts `accepted` there.
fn project(name: &str, accepted: &[&str]) -> std::path::PathBuf {
    let at = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(&at);
    routed(&at, &[("jellyfin", Kind::Jellyfin, GATES_KEY)]);
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

/// Where the request service reaches the media server: at `host`, `port` and `base`, with
/// `key`.
fn link(host: &str, port: u16, base: &str, key: &str) -> Answer {
    requests::answered(&MediaServerLink {
        at: Endpoint {
            host: host.to_owned(),
            port,
            base: base.to_owned(),
        },
        key: key.to_owned(),
    })
}

/// Linked at the gate with `key`.
fn at_the_gate(key: &str) -> Answer {
    link("request-gate", PORT, "/jellyfin", key)
}

/// A household whose media server, moved into its plugin, has been set up, whose request
/// service answers each of `initialised` in turn when asked whether it is set up, says it
/// reaches the media server as `linked`, and answers a new link with `relinked`.
fn serving(initialised: &[bool], linked: Answer, relinked: Answer) -> Arc<Fake> {
    Fake::by_path_in_turn(vec![
        (
            media::identity("startup_completed"),
            vec![media::answered(&true)],
        ),
        (
            media::identity("replace_admin_password"),
            vec![media::taken()],
        ),
        (
            media::serve("filed_as"),
            vec![media::answered(&[GATES_KEY])],
        ),
        (requests::intake("establish"), vec![requests::taken()]),
        (
            requests::intake("configure_identity"),
            vec![requests::taken()],
        ),
        (
            requests::intake("initialized"),
            initialised.iter().map(requests::answered).collect(),
        ),
        (requests::intake("media_server_link"), vec![linked]),
        (requests::intake("link_media_server"), vec![relinked]),
        (
            requests::intake("telling"),
            vec![requests::answered(&crate::seed::wanted_telling())],
        ),
        (requests::intake("tell"), vec![requests::taken()]),
    ])
}

/// Where the request service was told to sign the household in through, in `http`.
fn signed_in_at(http: &Fake) -> Vec<String> {
    requests::told::<IdentitySource>(&http.requests(), "configure_identity", "source")
        .into_iter()
        .map(|source| source.at)
        .collect()
}

/// The keys the request service was told to reach the media server with, in `http`.
fn relinked_with(http: &Fake) -> Vec<String> {
    requests::told(&http.requests(), "link_media_server", "key")
}

/// The media server's address as the gate answers for it.
fn through_the_gate() -> String {
    format!("http://request-gate:{PORT}/jellyfin")
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
    let mut ctx = asking(seed_ctx(true, bytes, Some(recorded_admin(name))).with_http(http.clone()));
    ctx.dry_run = rehearsing;
    seeded_with(&ctx, services, project).await
}

/// `ctx` asking the media server and the request service through their adapters.
fn asking(ctx: Ctx) -> Ctx {
    let mut ctx = media::asking(ctx);
    ctx.seams.engine = Arc::new(media::running().alongside(REQUESTS.running()));
    ctx
}

/// Both halves of the identity step in `ctx`, the request service moved into its plugin
/// as well as the media server.
async fn seeded_with(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    project: &std::path::Path,
) -> Vec<Wiring> {
    let installed: Vec<_> = media::moved()
        .into_iter()
        .chain(REQUESTS.installed())
        .collect();
    let fillers = fillers_beside(services.to_vec(), &installed, project);
    let server = crate::app::targets::MediaServer::of(&fillers);
    let admin = super::super::identity::seed_media_server_admin(ctx, server.as_ref()).await;
    super::super::seed_request_identity(
        ctx,
        services,
        &fillers,
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
    let http = serving(
        &[false, true],
        at_the_gate("a-placeholder"),
        requests::taken(),
    );

    let wirings = seeded("linking-fresh", &stack(true), &at, &http, true, false).await;

    assert_eq!(
        state_of(&wirings, crate::seed::IDENTITY),
        Some(State::Wired)
    );
    assert_eq!(state_of(&wirings, LINK), Some(State::Wired));
    assert_eq!(signed_in_at(&http), vec![through_the_gate()]);
    assert_eq!(
        requests::told::<Endpoint>(&http.requests(), "link_media_server", "at"),
        vec![Endpoint {
            host: "request-gate".to_owned(),
            port: PORT,
            base: "/jellyfin".to_owned(),
        }]
    );
    assert_eq!(relinked_with(&http), vec![minted()]);
    assert_eq!(accepted(&at), Some(accepting(&[&minted()])));
}

/// A token the request service holds at the gate, which the gate accepts, is left.
#[tokio::test]
async fn a_link_both_sides_hold_is_left_alone() {
    let at = project("linking-held", &["held"]);
    let http = serving(&[true], at_the_gate("held"), requests::taken());

    let wirings = seeded("linking-held", &stack(true), &at, &http, true, false).await;

    assert_eq!(state_of(&wirings, LINK), Some(State::AlreadyWired));
    assert_eq!(accepted(&at), Some(accepting(&["held"])));
}

/// A request service set up at the media server's own address is moved to the gate.
#[tokio::test]
async fn a_request_service_linked_directly_is_moved_to_the_gate() {
    let at = project("linking-direct", &[]);
    let http = serving(
        &[true],
        link("jellyfin", 8096, "", "seerrs-own"),
        requests::taken(),
    );

    let wirings = seeded("linking-direct", &stack(true), &at, &http, true, false).await;

    assert_eq!(state_of(&wirings, LINK), Some(State::Wired));
    assert_eq!(accepted(&at), Some(accepting(&[&minted()])));
}

/// A stack without the gate has no link to hold, and sets the request service up at
/// the media server's own address.
#[tokio::test]
async fn without_the_gate_there_is_no_link() {
    let at = project("linking-ungated", &[]);
    let http = serving(&[false, true], at_the_gate("x"), requests::taken());

    let wirings = seeded("linking-ungated", &stack(false), &at, &http, true, false).await;

    assert_eq!(state_of(&wirings, LINK), None);
    assert_eq!(signed_in_at(&http), vec!["http://jellyfin:8096".to_owned()]);
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
        let http = serving(&[initialised], linked, requests::taken());

        let wirings = seeded(name, &stack(true), &at, &http, true, true).await;

        assert_eq!(state_of(&wirings, LINK), Some(expected), "{name}");
        assert!(relinked_with(&http).is_empty(), "{name}");
        assert_eq!(accepted(&at), None, "{name}");
    }
}

/// A request service that will not take the token says so, and the gate keeps
/// accepting what it did.
#[tokio::test]
async fn a_token_the_request_service_refuses_is_said() {
    let at = project("linking-refused", &["older"]);
    let http = serving(&[true], at_the_gate("stale"), requests::refusing());

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
    let http = serving(&[true], requests::refusing(), requests::taken());
    let wirings = seeded("linking-unread", &stack(true), &at, &http, true, false).await;
    assert!(
        matches!(state_of(&wirings, LINK), Some(State::Failed { .. })),
        "{wirings:?}"
    );

    let at = project("linking-unrandom", &[]);
    let http = serving(&[true], at_the_gate("stale"), requests::taken());
    let wirings = seeded("linking-unrandom", &stack(true), &at, &http, false, false).await;
    assert_eq!(
        state_of(&wirings, LINK),
        Some(State::Failed {
            detail: "no randomness was available to generate a token".to_owned(),
        })
    );

    let at = project("linking-unwritable", &[]);
    let _ = std::fs::create_dir_all(tokens_file(&at).join("blocked"));
    let http = serving(&[true], at_the_gate("stale"), requests::taken());
    let wirings = seeded("linking-unwritable", &stack(true), &at, &http, true, false).await;
    let detail = match state_of(&wirings, LINK) {
        Some(State::Failed { detail }) => detail,
        other => format!("{other:?}"),
    };
    assert!(
        detail.starts_with("the tokens could not be handed to the request gate: ")
            && detail.contains(&tokens_file(&at).display().to_string()),
        "{detail}"
    );
    assert!(relinked_with(&http).is_empty());
}

/// An old token the gate cannot be told to stop accepting is said.
#[tokio::test]
async fn an_old_token_that_cannot_be_retired_is_said() {
    let at = project("linking-unretired", &[]);
    let _ = std::fs::create_dir_all(tokens_file(&at).join("blocked"));
    let both = accepting(&["older", &minted()]).written();
    let http = serving(&[true], at_the_gate("stale"), requests::taken());
    let ctx = asking(
        seed_ctx(
            true,
            Some(vec![0xab; crate::secret::SECRET_BYTES]),
            Some(recorded_admin("linking-unretired")),
        )
        .with_http(http.clone())
        .with_filesystem(lemonfiber_fixtures::files::Files::at(vec![
            (tokens_file(&at), &both),
            (
                crate::app::gating::path(&at, File::Upstreams),
                &admitting(&[("jellyfin", Kind::Jellyfin, GATES_KEY)]),
            ),
        ])),
    );

    let wirings = seeded_with(&ctx, &stack(true), &at).await;

    let detail = match state_of(&wirings, LINK) {
        Some(State::Failed { detail }) => detail,
        other => format!("{other:?}"),
    };
    assert!(
        detail.starts_with("the tokens could not be handed to the request gate: "),
        "{detail}"
    );
}

/// Where the request service could not be set up, it is not handed a link either.
#[tokio::test]
async fn a_request_service_not_set_up_is_not_linked() {
    let at = project("linking-unset", &[]);
    let http = serving(&[false], at_the_gate("x"), requests::taken());

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
