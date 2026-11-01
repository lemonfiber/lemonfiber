//! The request service pointed at the media server through the request gate, and handed the
//! token it reaches the media server with after its setup.

use crate::app::gating::PORT;
use crate::ports::digest::sha256;

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

/// The hash of each of `tokens`, as the gate accepts them.
fn accepting(tokens: &[&str]) -> Vec<String> {
    tokens.iter().map(|token| sha256(token)).collect()
}

/// The key the gate presents on its media server route, filed under its name there.
const GATES_KEY: &str = "gates-key";

/// A stack directory, and a gate routing to the media server under [`GATES_KEY`] and
/// accepting each of `accepted` there.
fn project(name: &str, accepted: &[&str]) -> (std::path::PathBuf, Arc<Gate>) {
    let at = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(&at);
    (at, Arc::new(media_route(accepted)))
}

/// A gate routing to the media server under [`GATES_KEY`], accepting each of `accepted`.
fn media_route(accepted: &[&str]) -> Gate {
    let mut route = gated_route("jellyfin", None, GATES_KEY);
    route.tokens = accepting(accepted);
    Gate::admitting(vec![route])
}

/// What `gate` accepts on the media server's route.
fn accepted(gate: &Gate) -> Vec<String> {
    gate.tokens("jellyfin").unwrap_or_default()
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

/// Both halves of the identity step over `http`, on `services` under `project` beside
/// `gate`.
async fn seeded(
    name: &str,
    services: &[lemonfiber_manifest::Service],
    (project, gate): (&std::path::Path, &Arc<Gate>),
    http: &Arc<Fake>,
    random: bool,
    rehearsing: bool,
) -> Vec<Wiring> {
    let bytes = random.then(|| vec![0xab; crate::secret::SECRET_BYTES]);
    let mut ctx = asking(
        seed_ctx(true, bytes, Some(recorded_admin(name))).with_http(http.clone()),
        project,
        gate,
    );
    ctx.dry_run = rehearsing;
    seeded_with(&ctx, services, project).await
}

/// `ctx` asking the media server and the request service through their adapters, and
/// `gate` under `project` over its contract.
fn asking(ctx: Ctx, project: &std::path::Path, gate: &Arc<Gate>) -> Ctx {
    let mut ctx = media::asking(ctx);
    ctx.seams.engine = Arc::new(media::running().alongside(REQUESTS.running()));
    crate::test_support::gated(ctx, project, gate)
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
    let fillers = guarded_beside(services.to_vec(), &installed, project);
    let server = crate::app::targets::MediaServer::of(&fillers);
    let admin = super::super::identity::seed_media_server_admin(ctx, server.as_ref()).await;
    super::super::seed_request_identity(
        ctx,
        services,
        &fillers,
        &crate::baseline::Baseline::new(),
        server.as_ref(),
        admin,
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
    let (at, gate) = project("linking-fresh", &[]);
    let http = serving(
        &[false, true],
        at_the_gate("a-placeholder"),
        requests::taken(),
    );

    let wirings = seeded(
        "linking-fresh",
        &stack(true),
        (&at, &gate),
        &http,
        true,
        false,
    )
    .await;

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
    assert_eq!(accepted(&gate), accepting(&[&minted()]));
}

/// A token the request service holds at the gate, which the gate accepts, is left.
#[tokio::test]
async fn a_link_both_sides_hold_is_left_alone() {
    let (at, gate) = project("linking-held", &["held"]);
    let http = serving(&[true], at_the_gate("held"), requests::taken());

    let wirings = seeded(
        "linking-held",
        &stack(true),
        (&at, &gate),
        &http,
        true,
        false,
    )
    .await;

    assert_eq!(state_of(&wirings, LINK), Some(State::AlreadyWired));
    assert_eq!(accepted(&gate), accepting(&["held"]));
}

/// A request service set up at the media server's own address is moved to the gate.
#[tokio::test]
async fn a_request_service_linked_directly_is_moved_to_the_gate() {
    let (at, gate) = project("linking-direct", &[]);
    let http = serving(
        &[true],
        link("jellyfin", 8096, "", "seerrs-own"),
        requests::taken(),
    );

    let wirings = seeded(
        "linking-direct",
        &stack(true),
        (&at, &gate),
        &http,
        true,
        false,
    )
    .await;

    assert_eq!(state_of(&wirings, LINK), Some(State::Wired));
    assert_eq!(accepted(&gate), accepting(&[&minted()]));
}

/// A stack without the gate has no link to hold, and sets the request service up at
/// the media server's own address.
#[tokio::test]
async fn without_the_gate_there_is_no_link() {
    let (at, gate) = project("linking-ungated", &[]);
    let http = serving(&[false, true], at_the_gate("x"), requests::taken());

    let wirings = seeded(
        "linking-ungated",
        &stack(false),
        (&at, &gate),
        &http,
        true,
        false,
    )
    .await;

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
        let (at, gate) = project(name, &[]);
        let http = serving(&[initialised], linked, requests::taken());

        let wirings = seeded(name, &stack(true), (&at, &gate), &http, true, true).await;

        assert_eq!(state_of(&wirings, LINK), Some(expected), "{name}");
        assert!(relinked_with(&http).is_empty(), "{name}");
        assert!(accepted(&gate).is_empty(), "{name}");
    }
}

/// A request service that will not take the token says so, and the gate keeps
/// accepting what it did.
#[tokio::test]
async fn a_token_the_request_service_refuses_is_said() {
    let (at, gate) = project("linking-refused", &["older"]);
    let http = serving(&[true], at_the_gate("stale"), requests::refusing());

    let wirings = seeded(
        "linking-refused",
        &stack(true),
        (&at, &gate),
        &http,
        true,
        false,
    )
    .await;

    let detail = match state_of(&wirings, LINK) {
        Some(State::Failed { detail }) => detail,
        other => format!("{other:?}"),
    };
    assert!(
        detail.starts_with("Seerr did not take the gate's Jellyfin token: "),
        "{detail}"
    );
    assert_eq!(accepted(&gate), accepting(&["older", &minted()]));
}

/// A link that cannot be read, a token that cannot be minted, and tokens the gate
/// cannot be handed each fail the link rather than call it done.
#[tokio::test]
async fn a_link_that_cannot_be_made_fails() {
    let (at, gate) = project("linking-unread", &[]);
    let http = serving(&[true], requests::refusing(), requests::taken());
    let wirings = seeded(
        "linking-unread",
        &stack(true),
        (&at, &gate),
        &http,
        true,
        false,
    )
    .await;
    assert!(
        matches!(state_of(&wirings, LINK), Some(State::Failed { .. })),
        "{wirings:?}"
    );

    let (at, gate) = project("linking-unrandom", &[]);
    let http = serving(&[true], at_the_gate("stale"), requests::taken());
    let wirings = seeded(
        "linking-unrandom",
        &stack(true),
        (&at, &gate),
        &http,
        false,
        false,
    )
    .await;
    assert_eq!(
        state_of(&wirings, LINK),
        Some(State::Failed {
            detail: "no randomness was available to generate a token".to_owned(),
        })
    );

    let (at, _) = project("linking-unwritable", &[]);
    let gate = Arc::new(media_route(&[]).refusing(&["admit_tokens"]));
    let http = serving(&[true], at_the_gate("stale"), requests::taken());
    let wirings = seeded(
        "linking-unwritable",
        &stack(true),
        (&at, &gate),
        &http,
        true,
        false,
    )
    .await;
    let detail = match state_of(&wirings, LINK) {
        Some(State::Failed { detail }) => detail,
        other => format!("{other:?}"),
    };
    assert!(
        detail.starts_with("the tokens could not be handed to the request gate: "),
        "{detail}"
    );
    assert!(relinked_with(&http).is_empty());
    assert!(accepted(&gate).is_empty());
}

/// An old token the gate cannot be told to stop accepting is said.
#[tokio::test]
async fn an_old_token_that_cannot_be_retired_is_said() {
    let (at, _) = project("linking-unretired", &[]);
    let gate = Arc::new(media_route(&["older", &minted()]).refusing(&["admit_tokens"]));
    let http = serving(&[true], at_the_gate("stale"), requests::taken());
    let ctx = asking(
        seed_ctx(
            true,
            Some(vec![0xab; crate::secret::SECRET_BYTES]),
            Some(recorded_admin("linking-unretired")),
        )
        .with_http(http.clone()),
        &at,
        &gate,
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
    assert_eq!(accepted(&gate), accepting(&["older", &minted()]));
}

/// Where the request service could not be set up, it is not handed a link either.
#[tokio::test]
async fn a_request_service_not_set_up_is_not_linked() {
    let (at, gate) = project("linking-unset", &[]);
    let http = serving(&[false], at_the_gate("x"), requests::taken());

    let wirings = seeded(
        "linking-unset",
        &stack(true),
        (&at, &gate),
        &http,
        true,
        false,
    )
    .await;

    assert!(
        matches!(
            state_of(&wirings, crate::seed::IDENTITY),
            Some(State::Failed { .. })
        ),
        "{wirings:?}"
    );
    assert_eq!(state_of(&wirings, LINK), None);
}
