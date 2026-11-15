//! The request service signed in at the gate's Plex route in each mode.

use super::*;
#[tokio::test]
async fn managed_signs_the_owner_in_with_the_managed_accounts_token_at_the_plex_route() {
    let http = signing(&[KEY], media::taken());
    let held = Gate::admitting(vec![plex_route(&[sha256(KEY)])]);
    let (ctx, at, _) = plex_ctx("plex-sign-managed", Some("managed"), held, http.clone());

    let state = signed(&ctx, &at, &http).await;

    assert_eq!(state, State::AlreadyWired);
    let established = asked_with(&http, "establish");
    assert_eq!(established.len(), 1, "{established:?}");
    assert!(
        established.iter().all(|body| {
            body.pointer("/source/at")
                .and_then(serde_json::Value::as_str)
                == Some("http://request-gate:32400")
                && body
                    .pointer("/source/protocol")
                    .and_then(serde_json::Value::as_str)
                    == Some("plex")
                && body.to_string().contains(KEY)
        }),
        "{established:?}"
    );
}

#[tokio::test]
async fn owner_carries_no_credential_and_waits_on_the_operators_own_sign_in() {
    let http = signing(&[KEY], media::refusing());
    let held = Gate::admitting(vec![plex_route(&[])]);
    let (ctx, at, _) = plex_ctx("plex-sign-owner", Some("owner"), held, http.clone());

    let state = signed(&ctx, &at, &http).await;

    assert!(matches!(state, State::Skipped { .. }), "{state:?}");
    let established = asked_with(&http, "establish");
    assert!(
        established.iter().all(|body| body
            .pointer("/source/credential")
            .is_none_or(serde_json::Value::is_null)),
        "{established:?}"
    );
}

#[tokio::test]
async fn fronted_mints_a_gate_token_signs_the_owner_in_with_it_and_admits_it_alone() {
    let http = signing(&[KEY], media::taken());
    let held = Gate::admitting(vec![plex_route(&[sha256(KEY)])]);
    let (ctx, at, gate) = plex_ctx("plex-sign-fronted", Some("fronted"), held, http.clone());
    let token = crate::secret::generate(&FixedRandom(Some(vec![0x11; 24]))).unwrap_or_default();

    let state = signed(&ctx, &at, &http).await;

    assert_eq!(state, State::AlreadyWired);
    assert_eq!(gate.tokens(SERVER), Some(vec![sha256(&token)]));
    assert!(asked_with(&http, "establish")
        .iter()
        .all(|body| body.to_string().contains(&token)));
}

#[tokio::test]
async fn fronted_with_a_gate_token_admitted_signs_in_with_none_and_one_refused_is_taken_back() {
    let kept_http = signing(&[KEY], media::taken());
    let kept = Gate::admitting(vec![plex_route(&[sha256("held")])]);
    let (ctx, at, gate) = plex_ctx("plex-sign-kept", Some("fronted"), kept, kept_http.clone());

    assert_eq!(signed(&ctx, &at, &kept_http).await, State::AlreadyWired);
    assert_eq!(gate.tokens(SERVER), Some(vec![sha256("held")]));

    let refused_http = signing(
        &[KEY],
        media::refused(lemonfiber_contract::wire::Kind::Unauthorised),
    );
    let fresh = Gate::admitting(vec![plex_route(&[sha256(KEY)])]);
    let (ctx, at, gate) = plex_ctx(
        "plex-sign-refused",
        Some("fronted"),
        fresh,
        refused_http.clone(),
    );

    let state = signed(&ctx, &at, &refused_http).await;

    assert!(matches!(state, State::Failed { .. }), "{state:?}");
    assert_eq!(gate.tokens(SERVER), Some(vec![sha256(KEY)]));
}

#[tokio::test]
async fn without_the_gate_or_a_key_it_holds_the_request_service_is_not_signed_in() {
    let http = signing(&[], media::taken());
    let (ctx, at, _) = plex_ctx(
        "plex-sign-unheld",
        Some("managed"),
        Gate::default(),
        http.clone(),
    );
    let fillers = fillers(&at);
    let server = crate::app::targets::MediaServer::of(&fillers);

    let unheld = signed(&ctx, &at, &http).await;
    let ungated = match &server {
        Some(server) => {
            crate::seed::run::plex_linking::signed_in(
                &ctx,
                &fillers,
                &requests(&http),
                server,
                false,
            )
            .await
            .state
        }
        None => State::Unmanaged,
    };

    assert!(matches!(unheld, State::Skipped { .. }), "{unheld:?}");
    assert!(matches!(ungated, State::Skipped { .. }), "{ungated:?}");
    assert!(asked_with(&http, "establish").is_empty());
}

/// The state signing the request service in comes to over `ctx`, beside `gate`'s.
async fn signed_beside(
    name: &str,
    mode: &str,
    gate: Gate,
    dry: bool,
) -> (State, Arc<Gate>, Arc<Fake>) {
    let http = signing(&[KEY], media::taken());
    let (mut ctx, at, gate) = plex_ctx(name, Some(mode), gate, http.clone());
    ctx.dry_run = dry;
    (signed(&ctx, &at, &http).await, gate, http)
}

#[tokio::test]
async fn a_mode_naming_none_fails_naming_the_three() {
    let (state, _, http) = signed_beside(
        "plex-sign-unnamed",
        "shared",
        Gate::admitting(vec![plex_route(&[])]),
        false,
    )
    .await;

    assert!(
        matches!(&state, State::Failed { detail } if detail.contains("shared is none of them")),
        "{state:?}"
    );
    assert!(asked_with(&http, "establish").is_empty());
}

#[tokio::test]
async fn fronted_rehearses_minting_a_token_and_writes_nothing() {
    let (state, gate, http) = signed_beside(
        "plex-sign-rehearsed",
        "fronted",
        Gate::admitting(vec![plex_route(&[sha256(KEY)])]),
        true,
    )
    .await;

    assert!(matches!(state, State::WouldWire { .. }), "{state:?}");
    assert_eq!(gate.tokens(SERVER), Some(vec![sha256(KEY)]));
    assert!(asked_with(&http, "establish").is_empty());
}

#[tokio::test]
async fn fronted_with_no_route_or_no_randomness_or_a_gate_refusing_tokens_signs_nobody_in() {
    let (unheld, _, _) =
        signed_beside("plex-sign-no-route", "fronted", Gate::default(), false).await;
    let (refused, gate, http) = signed_beside(
        "plex-sign-gate-refuses",
        "fronted",
        Gate::admitting(vec![plex_route(&[sha256(KEY)])]).refusing(&["admit_tokens"]),
        false,
    )
    .await;
    let http_random = signing(&[KEY], media::taken());
    let (ctx, at, _) = plex_ctx(
        "plex-sign-no-randomness",
        Some("fronted"),
        Gate::admitting(vec![plex_route(&[sha256(KEY)])]),
        http_random.clone(),
    );
    let ctx = ctx.with_random(Arc::new(FixedRandom(None)));
    let unrandom = signed(&ctx, &at, &http_random).await;

    assert!(matches!(unheld, State::Skipped { .. }), "{unheld:?}");
    assert!(matches!(refused, State::Failed { .. }), "{refused:?}");
    assert_eq!(gate.tokens(SERVER), Some(vec![sha256(KEY)]));
    assert!(asked_with(&http, "establish").is_empty());
    assert!(
        matches!(&unrandom, State::Failed { detail } if detail.contains("randomness")),
        "{unrandom:?}"
    );
}

#[tokio::test]
async fn the_request_services_page_offers_members_plex_sign_in_in_owner_and_fronted_alone() {
    for (mode, offered, answer) in [
        ("managed", false, media::taken()),
        ("owner", true, media::refusing()),
        ("fronted", true, media::taken()),
    ] {
        let http = signing(&[KEY], answer);
        let held = Gate::admitting(vec![plex_route(&[sha256(KEY)])]);
        let name = format!("plex-sign-offered-{mode}");
        let (ctx, at, _) = plex_ctx(&name, Some(mode), held, http.clone());

        signed(&ctx, &at, &http).await;

        let established = asked_with(&http, "establish");
        assert!(!established.is_empty(), "{mode}");
        assert!(
            established.iter().all(|body| body
                .pointer("/source/members_sign_in")
                .and_then(serde_json::Value::as_bool)
                == Some(offered)),
            "{mode}: {established:?}"
        );
    }
}
