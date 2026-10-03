//! The request gate's tokens: the request service reaches each \*arr through the gate,
//! under a token the gate accepts as a hash and lemonfiber keeps no copy of.

use lemonfiber_sidecar::gate::{Accepted, File, Tokens, PORT};
use lemonfiber_sidecar::TokenHash;

use super::*;

/// What the fixed randomness renders to, which is every token minted here.
fn minted() -> String {
    crate::secret::render(&[0xab; crate::secret::SECRET_BYTES])
}

/// A stack with Sonarr and the request service and, where `gating`, the request gate.
fn stack(gating: bool) -> Vec<lemonfiber_manifest::Service> {
    let mut services = vec![arr("sonarr", 8989, "tv"), seerr_svc()];
    if gating {
        services.push(manifest_service("request-gate", None, Some(PORT)));
    }
    services
}

/// A stack directory where Sonarr has written its key and, where `accepted` names
/// some, the gate accepts those tokens on Sonarr's route.
fn project(name: &str, accepted: &[&str]) -> std::path::PathBuf {
    let at = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&at);
    let _ = store::write(
        &at.join("config/sonarr/config.xml"),
        "<Config><ApiKey>sonarr-own-key</ApiKey></Config>",
    );
    if !accepted.is_empty() {
        let _ = store::write(&tokens_file(&at), &accepting(accepted).written());
    }
    at
}

/// The gate accepting `tokens` on Sonarr's route.
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

/// Sonarr as the request service lists it: at `host` and `port` under `base`, with
/// `key`.
fn listed(host: &str, port: u16, base: &str, key: &str) -> String {
    serde_json::json!([{
        "id": 1,
        "name": "Sonarr (mine)",
        "hostname": host,
        "port": port,
        "baseUrl": base,
        "apiKey": key,
        "activeProfileId": 9,
    }])
    .to_string()
}

/// Sonarr held at the gate with `key`.
fn at_the_gate(key: &str) -> String {
    listed("request-gate", PORT, "/sonarr", key)
}

/// A stack whose Sonarr answers with a profile and a folder, and whose request service
/// lists each of `held` in turn for Sonarr, answering a registration with `added` and
/// a move with `moved`.
fn serving(held: &[&str], added: u16, moved: u16) -> Arc<Fake> {
    Fake::by_route_in_turn(vec![
        (
            Method::Get,
            "/qualityprofile",
            vec![Answer::reply(200, r#"[{"id":4,"name":"HD-1080p"}]"#)],
        ),
        (
            Method::Get,
            "/rootfolder",
            vec![Answer::reply(200, r#"[{"id":1,"path":"/data/media/tv"}]"#)],
        ),
        (
            Method::Get,
            "/settings/radarr",
            vec![Answer::reply(200, "[]")],
        ),
        (
            Method::Get,
            "/settings/sonarr",
            held.iter().map(|list| Answer::reply(200, *list)).collect(),
        ),
        (
            Method::Post,
            "/settings/sonarr/test",
            vec![Answer::reply(200, "")],
        ),
        (
            Method::Post,
            "/settings/sonarr",
            vec![Answer::reply(added, "")],
        ),
        (
            Method::Put,
            "/settings/sonarr/1",
            vec![Answer::reply(moved, "")],
        ),
    ])
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
    let bytes = random.then(|| vec![0xab; crate::secret::SECRET_BYTES]);
    let mut ctx = seed_ctx(None, true, Vec::new(), bytes, None).with_http(http.clone());
    ctx.dry_run = rehearsing;
    super::super::seed_fulfilment_targets(&ctx, services, Some(project)).await
}

/// The body of the one call made with `method`.
fn sent(http: &Fake, method: Method) -> String {
    http.requests()
        .into_iter()
        .find(|asked| asked.method == method)
        .and_then(|asked| asked.body)
        .unwrap_or_default()
}

/// The states the step reports, in order.
fn states(wirings: &[Wiring]) -> Vec<State> {
    wirings.iter().map(|wiring| wiring.state.clone()).collect()
}

/// A stack with the gate hands the request service Sonarr at the gate, under a token
/// minted for its route, and the gate that token's hash alone.
#[tokio::test]
async fn an_arr_is_handed_over_at_the_gate_under_a_new_token() {
    let at = project("tokens-fresh", &[]);
    let http = serving(&["[]", "[]", &at_the_gate(&minted())], 201, 200);

    let wirings = seeded(&stack(true), &at, &http, true, false).await;

    assert_eq!(states(&wirings), vec![State::Wired]);
    let body = sent(&http, Method::Post);
    assert!(
        body.contains("\"hostname\":\"request-gate\"")
            && body.contains(&format!("\"port\":{PORT}"))
            && body.contains("\"baseUrl\":\"/sonarr\"")
            && body.contains(&format!("\"apiKey\":\"{}\"", minted())),
        "{body}"
    );
    assert!(!body.contains("sonarr-own-key"), "{body}");
    assert_eq!(accepted(&at), Some(accepting(&[&minted()])));
}

/// A token the request service holds and the gate accepts is left as it is.
#[tokio::test]
async fn a_token_both_sides_hold_is_left_alone() {
    let at = project("tokens-held", &["held"]);
    let http = serving(&[&at_the_gate("held")], 201, 200);

    let wirings = seeded(&stack(true), &at, &http, true, false).await;

    assert_eq!(states(&wirings), vec![State::AlreadyWired]);
    assert!(!http
        .requests()
        .iter()
        .any(|asked| asked.method != Method::Get));
    assert_eq!(accepted(&at), Some(accepting(&["held"])));
}

/// A token the gate no longer accepts is replaced in place, and the gate is left
/// accepting the new one alone.
#[tokio::test]
async fn a_token_the_gate_refuses_is_replaced() {
    let at = project("tokens-refused", &["other"]);
    let http = serving(&[&at_the_gate("stale")], 201, 200);

    let wirings = seeded(&stack(true), &at, &http, true, false).await;

    assert_eq!(states(&wirings), vec![State::Wired]);
    assert!(sent(&http, Method::Put).contains(&minted()));
    assert_eq!(accepted(&at), Some(accepting(&[&minted()])));
}

/// Sonarr held at its own address is moved to the gate in place, keeping everything
/// the operator chose about it.
#[tokio::test]
async fn an_arr_held_at_its_own_address_is_moved_to_the_gate() {
    let at = project("tokens-moved", &[]);
    let http = serving(&[&listed("sonarr", 8989, "", "sonarr-own-key")], 201, 200);

    let wirings = seeded(&stack(true), &at, &http, true, false).await;

    assert_eq!(states(&wirings), vec![State::Wired]);
    let body = sent(&http, Method::Put);
    assert!(
        body.contains("\"hostname\":\"request-gate\"")
            && body.contains("\"baseUrl\":\"/sonarr\"")
            && body.contains("\"name\":\"Sonarr (mine)\"")
            && body.contains("\"activeProfileId\":9"),
        "{body}"
    );
    assert!(!http
        .requests()
        .iter()
        .any(|asked| asked.method == Method::Post && asked.url.ends_with("/settings/sonarr")));
}

/// A stack that no longer runs the gate moves Sonarr back to its own address, with
/// its own key.
#[tokio::test]
async fn removing_the_gate_moves_an_arr_back() {
    let at = project("tokens-ungated", &[]);
    let http = serving(&[&at_the_gate("held")], 201, 200);

    let wirings = seeded(&stack(false), &at, &http, true, false).await;

    assert_eq!(states(&wirings), vec![State::Wired]);
    let body = sent(&http, Method::Put);
    assert!(
        body.contains("\"hostname\":\"sonarr\"")
            && body.contains("\"baseUrl\":\"\"")
            && body.contains("\"apiKey\":\"sonarr-own-key\""),
        "{body}"
    );
}

/// A rehearsal says where Sonarr would move, and writes nothing to either side.
#[tokio::test]
async fn a_rehearsal_says_where_and_writes_nothing() {
    for (name, held, expected) in [
        (
            "tokens-would-move",
            listed("sonarr", 8989, "", "sonarr-own-key"),
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
        let http = serving(&[&held], 201, 200);

        let wirings = seeded(&stack(true), &at, &http, true, true).await;

        assert_eq!(states(&wirings), vec![expected], "{name}");
        assert!(
            !http
                .requests()
                .iter()
                .any(|asked| asked.method != Method::Get),
            "{name}"
        );
        assert_eq!(accepted(&at), None, "{name}");
    }
}

/// A request service that does not take the new token leaves the gate accepting the
/// old one beside it, so nothing that worked stops working.
#[tokio::test]
async fn a_token_the_request_service_refuses_leaves_both_accepted() {
    let at = project("tokens-unmoved", &["older"]);
    let http = serving(&[&at_the_gate("stale")], 201, 500);

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

/// Tokens the gate cannot be handed are not given to the request service either.
#[tokio::test]
async fn tokens_that_cannot_be_written_are_given_to_nobody() {
    let at = project("tokens-unwritable", &[]);
    let _ = std::fs::create_dir_all(tokens_file(&at).join("blocked"));
    let http = serving(&["[]"], 201, 200);

    let wirings = seeded(&stack(true), &at, &http, true, false).await;

    let detail = wirings
        .first()
        .and_then(|one| match &one.state {
            State::Failed { detail } => Some(detail.clone()),
            _ => None,
        })
        .unwrap_or_default();
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
        .any(|asked| asked.method != Method::Get));
}

/// Retiring an old token the gate cannot be told about is said, and the old token
/// stays accepted beside the one the request service holds.
#[tokio::test]
async fn an_old_token_that_cannot_be_retired_is_said() {
    let at = project("tokens-unretired", &[]);
    let _ = std::fs::create_dir_all(tokens_file(&at).join("blocked"));
    let http = serving(&[&at_the_gate("held")], 201, 200);
    let both = accepting(&["held", "older"]).written();
    let ctx = seed_ctx(None, true, Vec::new(), None, None)
        .with_http(http.clone())
        .with_filesystem(lemonfiber_fixtures::files::Files::at(vec![
            (
                at.join("config/sonarr/config.xml"),
                "<Config><ApiKey>sonarr-own-key</ApiKey></Config>",
            ),
            (tokens_file(&at), &both),
        ]));

    let wirings = super::super::seed_fulfilment_targets(&ctx, &stack(true), Some(&at)).await;

    assert_eq!(
        wirings
            .iter()
            .map(|one| one.connection.as_str())
            .collect::<Vec<_>>(),
        vec![
            "sonarr the app as a request target",
            "The request gate's tokens"
        ]
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
    let http = serving(&["[]"], 201, 200);

    let wirings = seeded(&stack(true), &at, &http, false, false).await;

    assert_eq!(
        states(&wirings),
        vec![State::Failed {
            detail: "no randomness was available to generate a token".to_owned(),
        }]
    );
    assert_eq!(accepted(&at), None);
}
