//! The request gate's tokens: the request service reaches each curator through the gate,
//! under a token the gate accepts as a hash and lemonfiber keeps no copy of.

use lemonfiber_sidecar::gate::{Accepted, File, Kind, Tokens, PORT};
use lemonfiber_sidecar::TokenHash;

use super::*;
use crate::ports::service::{
    Endpoint, FulfilmentTarget, QualityProfile, RegisteredFolder, RegisteredTarget,
};
use crate::test_support::curators::{self, TELEVISION};
use crate::test_support::requests;

/// What the fixed randomness renders to, which is every token minted here.
fn minted() -> String {
    crate::secret::render(&[0xab; crate::secret::SECRET_BYTES])
}

/// A stack with, where `gating`, the request gate: the curator and the request service
/// come in their plugins.
fn stack(gating: bool) -> Vec<lemonfiber_manifest::Service> {
    if gating {
        vec![request_gate()]
    } else {
        Vec::new()
    }
}

/// `services`, written at `project`, with the television curator and the request service
/// moved into their plugins.
fn moved(
    services: Vec<lemonfiber_manifest::Service>,
    project: &std::path::Path,
) -> crate::wiring::Fillers {
    let installed: Vec<_> = TELEVISION
        .installed()
        .into_iter()
        .chain(REQUESTS.installed())
        .collect();
    fillers_beside(services, &installed, project)
}

/// A stack directory where, where `accepted` names some, the gate accepts those tokens on
/// the curator's route, which it reaches the curator on with the key lemonfiber minted.
fn project(name: &str, accepted: &[&str]) -> std::path::PathBuf {
    let at = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(&at);
    routed(&at, &[("sonarr", Kind::Sonarr, MINTED_CURATOR_KEY)]);
    if !accepted.is_empty() {
        let _ = store::write(&tokens_file(&at), &accepting(accepted).written());
    }
    at
}

/// The gate accepting `tokens` on the curator's route.
fn accepting(tokens: &[&str]) -> Tokens {
    Tokens::of(vec![Accepted {
        route: "sonarr".to_owned(),
        tokens: tokens.iter().map(|token| TokenHash::of(token)).collect(),
    }])
}

/// Where the gate reads its tokens, under `project`.
fn tokens_file(project: &std::path::Path) -> std::path::PathBuf {
    crate::app::gating::path(project, File::Tokens)
}

/// What the gate accepts under `project`, where its file reads.
fn accepted(project: &std::path::Path) -> Option<Tokens> {
    std::fs::read_to_string(tokens_file(project))
        .ok()
        .and_then(|text| Tokens::read(&text).ok())
}

/// Where the request service reaches the curator: at `host` and `port` under `base`.
fn reached(host: &str, port: u16, base: &str) -> Endpoint {
    Endpoint {
        host: host.to_owned(),
        port,
        base: base.to_owned(),
    }
}

/// The curator as the request service lists it: at `host` and `port` under `base`, with
/// `key`.
fn listed(host: &str, port: u16, base: &str, key: &str) -> Answer {
    requests::answered(&[RegisteredTarget {
        id: "1".to_owned(),
        at: reached(host, port, base),
        key: key.to_owned(),
        kind: crate::ports::media::Kind::Tv,
    }])
}

/// Where the gate answers for the curator.
fn the_gate() -> Endpoint {
    reached("request-gate", PORT, "/sonarr")
}

/// The curator held at the gate with `key`.
fn at_the_gate(key: &str) -> Answer {
    listed("request-gate", PORT, "/sonarr", key)
}

/// The request service listing nothing.
fn nothing() -> Answer {
    requests::answered(&Vec::<RegisteredTarget>::new())
}

/// A stack whose curator answers with a profile and a folder, and whose request service
/// lists each of `held` in turn for the curator, answering a registration with `added` and
/// a move with `moved`.
fn serving(held: Vec<Answer>, added: Answer, moved: Answer) -> Arc<Fake> {
    Fake::by_path_in_turn(vec![
        (
            curators::curating(TELEVISION, "quality_profiles"),
            vec![curators::answered(&[QualityProfile {
                id: 4,
                name: "HD-1080p".to_owned(),
            }])],
        ),
        (
            curators::curating(TELEVISION, "root_folders"),
            vec![curators::answered(&[RegisteredFolder {
                id: "1".to_owned(),
                path: "/data/media/tv".to_owned(),
            }])],
        ),
        (requests::intake("fulfilment_targets"), held),
        (
            requests::intake("test_fulfilment_target"),
            vec![requests::taken()],
        ),
        (requests::intake("add_fulfilment_target"), vec![added]),
        (requests::intake("move_fulfilment_target"), vec![moved]),
    ])
}

/// A context over `http` asking the curator and the request service, the curator's minted
/// key kept, with randomness that renders to [`minted`] where `random` and none where not.
fn asking(name: &str, http: &Arc<Fake>, random: bool) -> Ctx {
    let bytes = random.then(|| vec![0xab; crate::secret::SECRET_BYTES]);
    let ctx = seed_ctx(true, Vec::new(), bytes, Some(recorded_admin(name))).with_http(http.clone());
    TELEVISION.minted(&ctx, MINTED_CURATOR_KEY);
    crate::test_support::adapted::asking(ctx, &[TELEVISION, REQUESTS])
}

/// Seed the request targets of `services` under `project`, over `http`, with
/// randomness that renders to [`minted`] where `random` and none where not.
async fn seeded(
    services: &[lemonfiber_manifest::Service],
    project: &std::path::Path,
    http: &Arc<Fake>,
    random: bool,
    rehearsing: bool,
) -> Vec<Wiring> {
    let name = project
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("tokens");
    let mut ctx = asking(name, http, random);
    ctx.dry_run = rehearsing;
    super::super::seed_fulfilment_targets(
        &ctx,
        services,
        &moved(services.to_vec(), project),
        Some(project),
    )
    .await
}

/// Each target the request service was asked to add, in `http`.
fn added(http: &Fake) -> Vec<FulfilmentTarget> {
    requests::told(&http.requests(), "add_fulfilment_target", "target")
}

/// Where and with what key the request service was asked to move the curator, in `http`.
fn moves(http: &Fake) -> Vec<(Endpoint, String)> {
    let sent = http.requests();
    requests::told::<Endpoint>(&sent, "move_fulfilment_target", "at")
        .into_iter()
        .zip(requests::told::<String>(
            &sent,
            "move_fulfilment_target",
            "key",
        ))
        .collect()
}

/// Whether the request service was asked to change anything, in `http`.
fn written(http: &Fake) -> bool {
    !added(http).is_empty() || !moves(http).is_empty()
}

/// The states the step reports, in order.
fn states(wirings: &[Wiring]) -> Vec<State> {
    wirings.iter().map(|wiring| wiring.state.clone()).collect()
}

/// A stack with the gate hands the request service the curator at the gate, under a token
/// minted for its route, and the gate that token's hash alone.
#[tokio::test]
async fn a_curator_is_handed_over_at_the_gate_under_a_new_token() {
    let at = project("tokens-fresh", &[]);
    let http = serving(
        vec![nothing(), nothing(), at_the_gate(&minted())],
        requests::taken(),
        requests::taken(),
    );

    let wirings = seeded(&stack(true), &at, &http, true, false).await;

    assert_eq!(states(&wirings), vec![State::Wired]);
    assert_eq!(
        added(&http)
            .into_iter()
            .map(|target| (target.at, target.key))
            .collect::<Vec<_>>(),
        vec![(the_gate(), minted())]
    );
    assert_eq!(accepted(&at), Some(accepting(&[&minted()])));
}

/// A token the request service holds and the gate accepts is left as it is.
#[tokio::test]
async fn a_token_both_sides_hold_is_left_alone() {
    let at = project("tokens-held", &["held"]);
    let http = serving(
        vec![at_the_gate("held")],
        requests::taken(),
        requests::taken(),
    );

    let wirings = seeded(&stack(true), &at, &http, true, false).await;

    assert_eq!(states(&wirings), vec![State::AlreadyWired]);
    assert!(!written(&http));
    assert_eq!(accepted(&at), Some(accepting(&["held"])));
}

/// A token the gate no longer accepts is replaced in place, and the gate is left
/// accepting the new one alone.
#[tokio::test]
async fn a_token_the_gate_refuses_is_replaced() {
    let at = project("tokens-refused", &["other"]);
    let http = serving(
        vec![at_the_gate("stale")],
        requests::taken(),
        requests::taken(),
    );

    let wirings = seeded(&stack(true), &at, &http, true, false).await;

    assert_eq!(states(&wirings), vec![State::Wired]);
    assert_eq!(moves(&http), vec![(the_gate(), minted())]);
    assert_eq!(accepted(&at), Some(accepting(&[&minted()])));
}

/// The curator held at its own address is moved to the gate in place rather than added a
/// second time.
#[tokio::test]
async fn a_curator_held_at_its_own_address_is_moved_to_the_gate() {
    let at = project("tokens-moved", &[]);
    let http = serving(
        vec![listed("sonarr", 8989, "", MINTED_CURATOR_KEY)],
        requests::taken(),
        requests::taken(),
    );

    let wirings = seeded(&stack(true), &at, &http, true, false).await;

    assert_eq!(states(&wirings), vec![State::Wired]);
    assert_eq!(moves(&http), vec![(the_gate(), minted())]);
    assert!(added(&http).is_empty());
}

/// A stack that no longer runs the gate moves the curator back to its own address, with
/// the key lemonfiber minted for it.
#[tokio::test]
async fn removing_the_gate_moves_a_curator_back() {
    let at = project("tokens-ungated", &[]);
    let http = serving(
        vec![at_the_gate("held")],
        requests::taken(),
        requests::taken(),
    );

    let wirings = seeded(&stack(false), &at, &http, true, false).await;

    assert_eq!(states(&wirings), vec![State::Wired]);
    assert_eq!(
        moves(&http),
        vec![(reached("sonarr", 8989, ""), MINTED_CURATOR_KEY.to_owned())]
    );
}

/// A rehearsal says where the curator would move, and writes nothing to either side.
#[tokio::test]
async fn a_rehearsal_says_where_and_writes_nothing() {
    for (name, held, expected) in [
        (
            "tokens-would-move",
            listed("sonarr", 8989, "", MINTED_CURATOR_KEY),
            State::WouldWire {
                yours: Some("at sonarr:8989".to_owned()),
                ours: Some("at the request gate".to_owned()),
            },
        ),
        (
            "tokens-would-replace",
            at_the_gate("stale"),
            State::WouldWire {
                yours: None,
                ours: Some("at the request gate".to_owned()),
            },
        ),
    ] {
        let at = project(name, &[]);
        let http = serving(vec![held], requests::taken(), requests::taken());

        let wirings = seeded(&stack(true), &at, &http, true, true).await;

        assert_eq!(states(&wirings), vec![expected], "{name}");
        assert!(!written(&http), "{name}");
        assert_eq!(accepted(&at), None, "{name}");
    }
}

/// A request service that does not take the new token leaves the gate accepting the
/// old one beside it, so nothing that worked stops working.
#[tokio::test]
async fn a_token_the_request_service_refuses_leaves_both_accepted() {
    let at = project("tokens-unmoved", &["older"]);
    let http = serving(
        vec![at_the_gate("stale")],
        requests::taken(),
        requests::refusing(),
    );

    let wirings = seeded(&stack(true), &at, &http, true, false).await;

    assert!(
        matches!(
            wirings.first().map(|one| &one.state),
            Some(State::Failed { .. })
        ),
        "{wirings:?}"
    );
    assert_eq!(accepted(&at), Some(accepting(&["older", &minted()])));
}

/// A curator the gate has no route to is not given to the request service at the gate,
/// and the gate is handed no token for it.
#[tokio::test]
async fn a_curator_the_gate_has_no_route_to_is_given_to_nobody() {
    let at = project("tokens-unrouted", &[]);
    let _ = std::fs::remove_file(crate::app::gating::path(&at, File::Upstreams));
    let http = serving(vec![nothing()], requests::taken(), requests::taken());

    let wirings = seeded(&stack(true), &at, &http, true, false).await;

    assert_eq!(
        states(&wirings),
        vec![State::Failed {
            detail: "the tokens could not be handed to the request gate: it admits no route \
                     sonarr"
                .to_owned(),
        }]
    );
    assert!(!written(&http));
    assert_eq!(accepted(&at), None);
}

/// Tokens the gate cannot be handed are not given to the request service either.
#[tokio::test]
async fn tokens_that_cannot_be_written_are_given_to_nobody() {
    let at = project("tokens-unwritable", &[]);
    let _ = std::fs::create_dir_all(tokens_file(&at).join("blocked"));
    let http = serving(vec![nothing()], requests::taken(), requests::taken());

    let wirings = seeded(&stack(true), &at, &http, true, false).await;

    let detail = wirings
        .first()
        .and_then(|one| match &one.state {
            State::Failed { detail } => Some(detail.clone()),
            _ => None,
        })
        .unwrap_or_default();
    assert!(
        detail.starts_with("the tokens could not be handed to the request gate: ")
            && detail.contains(&tokens_file(&at).display().to_string()),
        "{detail}"
    );
    assert!(!written(&http));
}

/// Retiring an old token the gate cannot be told about is said, and the old token
/// stays accepted beside the one the request service holds.
#[tokio::test]
async fn an_old_token_that_cannot_be_retired_is_said() {
    let at = project("tokens-unretired", &[]);
    let _ = std::fs::create_dir_all(tokens_file(&at).join("blocked"));
    let http = serving(
        vec![at_the_gate("held")],
        requests::taken(),
        requests::taken(),
    );
    let both = accepting(&["held", "older"]).written();
    let ctx = asking("tokens-unretired", &http, false).with_filesystem(
        lemonfiber_fixtures::files::Files::at(vec![
            (tokens_file(&at), &both),
            (
                crate::app::gating::path(&at, File::Upstreams),
                &admitting(&[("sonarr", Kind::Sonarr, MINTED_CURATOR_KEY)]),
            ),
        ]),
    );

    let wirings = super::super::seed_fulfilment_targets(
        &ctx,
        &stack(true),
        &moved(stack(true), &at),
        Some(&at),
    )
    .await;

    assert_eq!(
        wirings
            .iter()
            .map(|one| one.connection.as_str())
            .collect::<Vec<_>>(),
        vec!["Sonarr as a request target", "The request gate's tokens"]
    );
    assert!(
        matches!(
            wirings.get(1).map(|one| &one.state),
            Some(State::Failed { .. })
        ),
        "{wirings:?}"
    );
}

/// With no randomness, no token is minted and nothing is handed over.
#[tokio::test]
async fn without_randomness_no_token_is_minted() {
    let at = project("tokens-unrandom", &[]);
    let http = serving(vec![nothing()], requests::taken(), requests::taken());

    let wirings = seeded(&stack(true), &at, &http, false, false).await;

    assert_eq!(
        states(&wirings),
        vec![State::Failed {
            detail: "no randomness was available to generate a token".to_owned(),
        }]
    );
    assert_eq!(accepted(&at), None);
}

/// A route whose credential the gate presents is not the one lemonfiber knows the curator
/// by is never handed back to the gate with lemonfiber's: its tokens are left as they are,
/// and nothing is given to the request service.
#[tokio::test]
async fn a_route_presenting_a_credential_lemonfiber_does_not_know_is_not_handed_back() {
    let at = project("tokens-unvouched", &[]);
    routed(&at, &[("sonarr", Kind::Sonarr, "a-key-nobody-wrote")]);
    let http = serving(vec![nothing()], requests::taken(), requests::taken());

    let wirings = seeded(&stack(true), &at, &http, true, false).await;

    assert_eq!(
        wirings.first().map(|one| &one.state),
        Some(&State::Failed {
            detail: "the tokens could not be handed to the request gate: lemonfiber does not \
                     hold the credential it presents on its route sonarr"
                .to_owned(),
        })
    );
    assert_eq!(accepted(&at), None);
    assert!(!written(&http));
}
