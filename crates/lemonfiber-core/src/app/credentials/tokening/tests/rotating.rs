//! Rotating a gate token: who it is handed to, and what a rotation that cannot land
//! leaves.

use super::*;

#[tokio::test]
async fn a_third_party_request_service_is_never_handed_a_token() {
    let http = serving("held", "linked", 200, &[200], 200);
    let (ctx, at) = scene(
        "tokens-third-party",
        true,
        &accepting(&["held"], &["linked"]),
        http.clone(),
        true,
    );
    let line = named(
        &held(&ctx, &stack(true), &fillers(true, Some(&at)), Some(&at)).await,
        SONARR,
    );
    let without_requests: Vec<_> = stack(true)
        .into_iter()
        .filter(|service| service.id != "seerr")
        .collect();
    let asked_before = http.requests().len();

    let rotation = crate::app::credentials::rotating::rotate(
        &ctx,
        &line,
        &without_requests,
        &fillers_beside(
            without_requests.clone(),
            &[crate::test_support::contracted(
                "intake",
                crate::test_support::CONTRACTED_REQUESTS,
                "request.intake",
            )],
            Some(&at),
        ),
        Some(&at),
    )
    .await;

    assert!(
        matches!(&rotation.settled, Settled::Unproven { detail } if detail.contains("does not hand")),
        "{rotation:?}"
    );
    assert_eq!(accepted(&at), Some(accepting(&["held"], &["linked"])));
    assert_eq!(http.requests().len(), asked_before);
}

/// A stack directory whose gate accepts `held` on Sonarr's route, and a context over
/// `http` reaching a first-party plugin's request service speaking `request.intake` in
/// place of the stack's own, holding its key where `keyed`; with the stack's services and
/// what its asks come to.
fn beside_contracted_requests(
    name: &str,
    keyed: bool,
    http: &Arc<Fake>,
) -> (
    Ctx,
    PathBuf,
    Vec<lemonfiber_manifest::Service>,
    crate::wiring::Fillers,
) {
    let (_, at) = scene(name, true, &accepting(&["held"], &[]), http.clone(), true);
    let ctx = crate::test_support::contracted_context(
        &at,
        crate::test_support::CONTRACTED_REQUESTS,
        keyed,
    )
    .settings(Settings::default())
    .build()
    .with_http(http.clone())
    .with_random(Arc::new(lemonfiber_fixtures::support::FixedRandom(Some(
        vec![0xab; crate::secret::SECRET_BYTES],
    ))));
    let services: Vec<_> = stack(true)
        .into_iter()
        .filter(|service| service.id != "seerr")
        .collect();
    let fillers = crate::test_support::stack()
        .manifest()
        .map(|mut manifest| {
            manifest.services = services.clone();
            crate::wiring::Fillers::trusting(
                &manifest,
                &[crate::test_support::contracted(
                    "intake",
                    crate::test_support::CONTRACTED_REQUESTS,
                    "request.intake",
                )],
                &crate::wiring::Chosen::default(),
                Some(&at),
                &crate::test_support::first_party("intake"),
            )
        })
        .unwrap_or_default();
    (ctx, at, services, fillers)
}

/// Sonarr's token's line, as the inventory reads it.
fn sonarr_line() -> Held {
    Held {
        name: SONARR.to_owned(),
        setting: "request-gate/tokens.json#sonarr".to_owned(),
        consumers: Vec::new(),
        location: String::new(),
        origin: crate::credential::Origin::Lemonfiber,
        from: crate::origin::Origin::Bundled,
        state: State::Active,
        fingerprint: None,
        advisory: None,
    }
}

/// **A first-party request service speaking `request.intake` is handed the new token over
/// it**, which the trust gate lets the stack's curator's token cross to, and the stack's
/// own request service's API is never asked.
#[tokio::test]
async fn a_first_party_request_service_is_handed_a_token_over_the_contract() {
    let target = serde_json::json!([{
        "id": "1",
        "at": { "host": crate::app::gating::SERVICE, "port": PORT, "base": "/sonarr" },
        "key": "held",
        "kind": "tv",
    }])
    .to_string();
    let http = Fake::by_path_in_turn(vec![
        ("/v1/fulfilment_targets", vec![Answer::reply(200, target)]),
        ("/v1/move_fulfilment_target", vec![Answer::reply(204, "")]),
        ("/v1/test_fulfilment_target", vec![Answer::reply(204, "")]),
    ]);
    let (ctx, at, services, fillers) =
        beside_contracted_requests("tokens-first-party", true, &http);

    let rotation = rotate(&ctx, &sonarr_line(), &services, &fillers, Some(&at)).await;

    assert!(
        matches!(rotation.settled, Settled::Replaced { .. }),
        "{rotation:?}"
    );
    assert_eq!(accepted(&at), Some(accepting(&[&minted()], &[])));
    let asked = http.requests();
    assert!(
        asked.iter().any(|one| one.url
            == format!(
                "{}test_fulfilment_target",
                crate::test_support::CONTRACTED_REQUESTS_AT
            )
            && one
                .body
                .as_deref()
                .is_some_and(|body| body.contains(&minted()))),
        "{asked:?}"
    );
    assert!(
        !asked.iter().any(|one| one.url.contains("/api/v1/")),
        "{asked:?}"
    );
}

/// **A request service speaking `request.intake` that cannot be asked over it is asked
/// nothing.** Its tokens have no line, a rotation hands it nothing, and the gate keeps
/// accepting what it accepted.
#[tokio::test]
async fn a_request_service_that_cannot_be_asked_over_the_contract_is_handed_nothing() {
    let http = Fake::always(Answer::reply(200, "[]"));
    let (ctx, at, services, fillers) = beside_contracted_requests("tokens-unasked", false, &http);

    let lines = held(&ctx, &services, &fillers, Some(&at)).await;
    let rotation = rotate(&ctx, &sonarr_line(), &services, &fillers, Some(&at)).await;

    assert!(lines.is_empty(), "{lines:?}");
    assert!(
        unproven(&rotation.settled).is_some_and(|said| said.ends_with("Run `lemonfiber seed`.")),
        "{rotation:?}"
    );
    assert_eq!(accepted(&at), Some(accepting(&["held"], &[])));
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

#[tokio::test]
async fn an_arr_token_is_replaced_once_the_request_service_proves_it() {
    let http = serving("held", "linked", 200, &[200], 200);
    let (ctx, at) = scene(
        "tokens-rotate-arr",
        true,
        &accepting(&["held"], &["linked"]),
        http.clone(),
        true,
    );
    let line = named(
        &held(&ctx, &stack(true), &fillers(true, Some(&at)), Some(&at)).await,
        SONARR,
    );

    let rotation = crate::app::credentials::rotating::rotate(
        &ctx,
        &line,
        &stack(true),
        &fillers(true, Some(&at)),
        Some(&at),
    )
    .await;

    assert!(
        matches!(rotation.settled, Settled::Replaced { .. }),
        "{rotation:?}"
    );
    assert_eq!(accepted(&at), Some(accepting(&[&minted()], &["linked"])));
    assert_eq!(
        rotation.consumers.first().map(|one| one.reach.clone()),
        Some(Reach::Updated)
    );
    assert!(http
        .requests()
        .iter()
        .any(|asked| asked.url.ends_with("/settings/sonarr/test")
            && asked
                .body
                .as_deref()
                .is_some_and(|body| body.contains(&minted()))));
}

#[tokio::test]
async fn an_arr_token_the_request_service_cannot_prove_is_taken_back() {
    let http = serving("held", "linked", 200, &[500], 200);
    let (ctx, at) = scene(
        "tokens-rotate-unproven",
        true,
        &accepting(&["held"], &[]),
        http.clone(),
        true,
    );
    let line = named(
        &held(&ctx, &stack(true), &fillers(true, Some(&at)), Some(&at)).await,
        SONARR,
    );

    let rotation = rotate(
        &ctx,
        &line,
        &stack(true),
        &fillers(true, Some(&at)),
        Some(&at),
    )
    .await;

    assert_eq!(
        unproven(&rotation.settled),
        Some(
            "the request service could not reach Sonarr through the gate with the new token, \
             so the old one was put back"
        )
    );
    assert_eq!(accepted(&at), Some(accepting(&["held"], &[])));
    let moves: Vec<String> = http
        .requests()
        .into_iter()
        .filter(|asked| asked.method == Method::Put)
        .filter_map(|asked| asked.body)
        .collect();
    assert_eq!(moves.len(), 2, "{moves:?}");
    assert!(moves
        .last()
        .is_some_and(|body| body.contains("\"apiKey\":\"held\"")));
}

#[tokio::test]
async fn the_jellyfin_token_is_replaced_once_the_request_service_keeps_it() {
    let http = serving("held", "linked", 200, &[200], 200);
    let (ctx, at) = scene(
        "tokens-rotate-jellyfin",
        true,
        &accepting(&["held"], &["linked"]),
        http,
        true,
    );
    let line = named(
        &held(&ctx, &stack(true), &fillers(true, Some(&at)), Some(&at)).await,
        JELLYFIN,
    );

    let rotation = rotate(
        &ctx,
        &line,
        &stack(true),
        &fillers(true, Some(&at)),
        Some(&at),
    )
    .await;

    assert!(
        matches!(rotation.settled, Settled::Replaced { .. }),
        "{rotation:?}"
    );
    assert_eq!(accepted(&at), Some(accepting(&["held"], &[&minted()])));

    let refused = serving("held", "linked", 200, &[200], 400);
    let (ctx, at) = scene(
        "tokens-rotate-unlinked",
        true,
        &accepting(&["held"], &["linked"]),
        refused,
        true,
    );
    let rotation = rotate(
        &ctx,
        &line,
        &stack(true),
        &fillers(true, Some(&at)),
        Some(&at),
    )
    .await;
    assert!(unproven(&rotation.settled).is_some_and(|said| said.contains("reach Jellyfin")));
    assert_eq!(accepted(&at), Some(accepting(&["held"], &["linked"])));
}

#[tokio::test]
async fn a_rotation_that_cannot_start_changes_nothing() {
    let http = serving("held", "linked", 200, &[200], 200);
    let (ctx, at) = scene(
        "tokens-rotate-unstarted",
        true,
        &accepting(&["held"], &[]),
        http.clone(),
        false,
    );
    let line = named(
        &held(&ctx, &stack(true), &fillers(true, Some(&at)), Some(&at)).await,
        SONARR,
    );

    let unrandom = rotate(
        &ctx,
        &line,
        &stack(true),
        &fillers(true, Some(&at)),
        Some(&at),
    )
    .await;
    assert_eq!(
        unproven(&unrandom.settled),
        Some("no randomness was available to generate a token")
    );
    let unrouted = rotate(&ctx, &line, &stack(true), &fillers(true, None), None).await;
    assert!(
        unproven(&unrouted.settled).is_some_and(|said| said.ends_with("Run `lemonfiber seed`."))
    );
    let mut gone = line.clone();
    gone.setting = "request-gate/tokens.json#radarr".to_owned();
    let unknown = rotate(
        &ctx,
        &gone,
        &stack(true),
        &fillers(true, Some(&at)),
        Some(&at),
    )
    .await;
    assert!(unproven(&unknown.settled).is_some());

    let (mut rehearsing, _) = scene(
        "tokens-rotate-rehearsed",
        true,
        &accepting(&["held"], &[]),
        http.clone(),
        true,
    );
    rehearsing.dry_run = true;
    let rehearsed = rotate(
        &rehearsing,
        &line,
        &stack(true),
        &fillers(true, Some(&at)),
        Some(&at),
    )
    .await;
    assert!(
        matches!(rehearsed.settled, Settled::Rehearsed { .. }),
        "{rehearsed:?}"
    );
    assert!(!http
        .requests()
        .iter()
        .any(|asked| asked.method != Method::Get));
    assert_eq!(accepted(&at), Some(accepting(&["held"], &[])));
}

#[tokio::test]
async fn a_rotation_the_service_or_the_file_stops_is_unproven() {
    for (name, moved, unheld, blocked) in [
        ("tokens-rotate-unmoved", 500, false, false),
        ("tokens-rotate-untargeted", 200, true, false),
        ("tokens-rotate-unwritable", 200, false, true),
    ] {
        let http = if unheld {
            Fake::by_route_in_turn(vec![
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
            ])
        } else {
            serving("held", "linked", moved, &[200], 200)
        };
        let (ctx, at) = scene(name, true, &accepting(&["held"], &[]), http, true);
        let line = Held {
            name: SONARR.to_owned(),
            setting: "request-gate/tokens.json#sonarr".to_owned(),
            consumers: Vec::new(),
            location: String::new(),
            origin: crate::credential::Origin::Lemonfiber,
            from: crate::origin::Origin::Bundled,
            state: State::Active,
            fingerprint: None,
            advisory: None,
        };
        let ctx = if blocked {
            let held_text = accepting(&["held"], &[]).written();
            let routes_text = routes().written();
            let _ = std::fs::remove_file(tokens_file(&at));
            let _ = std::fs::create_dir_all(tokens_file(&at).join("blocked"));
            ctx.with_filesystem(lemonfiber_fixtures::files::Files::at(vec![
                (tokens_file(&at), &held_text),
                (crate::app::gating::path(&at, File::Upstreams), &routes_text),
            ]))
        } else {
            ctx
        };

        let rotation = rotate(
            &ctx,
            &line,
            &stack(true),
            &fillers(true, Some(&at)),
            Some(&at),
        )
        .await;

        assert!(
            unproven(&rotation.settled).is_some(),
            "{name}: {rotation:?}"
        );
    }
}

#[tokio::test]
async fn an_old_token_the_gate_cannot_be_told_to_drop_is_said() {
    let http = serving("held", "linked", 200, &[200], 200);
    let (ctx, at) = scene(
        "tokens-rotate-unretired",
        true,
        &accepting(&[], &[]),
        http,
        true,
    );
    let both = accepting(&["held", &minted()], &[]).written();
    let routes_text = routes().written();
    let _ = std::fs::remove_file(tokens_file(&at));
    let _ = std::fs::create_dir_all(tokens_file(&at).join("blocked"));
    let ctx = ctx.with_filesystem(lemonfiber_fixtures::files::Files::at(vec![
        (tokens_file(&at), &both),
        (crate::app::gating::path(&at, File::Upstreams), &routes_text),
    ]));
    let line = named(
        &held(&ctx, &stack(true), &fillers(true, Some(&at)), Some(&at)).await,
        SONARR,
    );

    let rotation = rotate(
        &ctx,
        &line,
        &stack(true),
        &fillers(true, Some(&at)),
        Some(&at),
    )
    .await;

    assert!(
        unproven(&rotation.settled)
            .is_some_and(|said| said.starts_with("the tokens could not be written")),
        "{rotation:?}"
    );
}

#[tokio::test]
async fn a_request_service_that_stops_answering_mid_rotation_changes_nothing() {
    let http = Fake::by_route_in_turn(vec![(
        Method::Get,
        "/settings/sonarr",
        vec![Answer::Silent],
    )]);
    let (ctx, at) = scene(
        "tokens-rotate-unanswered",
        true,
        &accepting(&["held"], &[]),
        http,
        true,
    );
    let line = Held {
        name: SONARR.to_owned(),
        setting: "request-gate/tokens.json#sonarr".to_owned(),
        consumers: Vec::new(),
        location: String::new(),
        origin: crate::credential::Origin::Lemonfiber,
        from: crate::origin::Origin::Bundled,
        state: State::Active,
        fingerprint: None,
        advisory: None,
    };

    let rotation = rotate(
        &ctx,
        &line,
        &stack(true),
        &fillers(true, Some(&at)),
        Some(&at),
    )
    .await;

    assert!(unproven(&rotation.settled).is_some(), "{rotation:?}");
    assert_eq!(accepted(&at), Some(accepting(&["held"], &[])));
}
