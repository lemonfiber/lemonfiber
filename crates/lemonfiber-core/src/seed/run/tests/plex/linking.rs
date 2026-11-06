//! The request service pointed at the gate's Plex route, and set up through a claimed Plex.

use super::*;
#[tokio::test]
async fn the_request_service_is_pointed_at_the_plex_route_with_no_key() {
    let elsewhere = crate::ports::service::Endpoint {
        host: "plex".to_owned(),
        port: 32400,
        base: String::new(),
    };
    let link = |at: &crate::ports::service::Endpoint| {
        media::answered(&serde_json::json!({"at": at, "key": ""}))
    };
    let http = Fake::by_path_in_turn(vec![
        (
            intake_at("media_server_link"),
            vec![
                link(&elsewhere),
                link(&crate::seed::run::plex_linking::route()),
            ],
        ),
        (intake_at("link_media_server"), vec![media::taken()]),
    ]);
    let (ctx, at, _) = plex_ctx("plex-linked", None, Gate::default(), http.clone());
    let Some(server) = server(&at) else {
        return;
    };
    let asker = server
        .asked_by
        .clone()
        .unwrap_or_else(|| server.served.clone());

    let first =
        crate::seed::run::plex_linking::linked(&ctx, &requests(&http), &asker, &server).await;
    let second =
        crate::seed::run::plex_linking::linked(&ctx, &requests(&http), &asker, &server).await;

    assert_eq!(first.state, State::Wired);
    assert_eq!(second.state, State::AlreadyWired);
    let written = asked_with(&http, "link_media_server");
    assert_eq!(written.len(), 1, "{written:?}");
    assert!(written
        .iter()
        .all(
            |body| body.get("key").and_then(serde_json::Value::as_str) == Some("")
                && body.pointer("/at/port").and_then(serde_json::Value::as_u64) == Some(32400)
        ));
}

#[tokio::test]
async fn a_claimed_server_mints_no_administrator() {
    let http = plex(&[], None, Vec::new());
    let (ctx, at, _) = plex_ctx("plex-no-admin", None, Gate::default(), http.clone());
    let server = server(&at);

    let admin = crate::seed::run::identity::seed_media_server_admin(&ctx, server.as_ref()).await;

    assert!(admin.is_some() || server.as_ref().is_some_and(|one| one.asked_by.is_none()));
    assert!(!http
        .requests()
        .iter()
        .any(|one| one.url.contains("/identity.source/")));
}

#[tokio::test]
async fn a_claimed_plex_links_the_request_service_through_the_gate_with_no_administrator() {
    let elsewhere = crate::ports::service::Endpoint {
        host: "plex".to_owned(),
        port: 32400,
        base: String::new(),
    };
    let http = plex(
        &[KEY],
        None,
        vec![
            (intake_at("establish"), vec![media::taken()]),
            (intake_at("initialized"), vec![media::answered(&true)]),
            (
                intake_at("media_server_link"),
                vec![media::answered(
                    &serde_json::json!({"at": elsewhere, "key": ""}),
                )],
            ),
            (intake_at("link_media_server"), vec![media::taken()]),
            (
                intake_at("telling"),
                vec![media::answered(&serde_json::json!({
                    "enabled": false, "occasions": [], "others": false
                }))],
            ),
            (intake_at("tell"), vec![media::taken()]),
        ],
    );
    let held = Gate::admitting(vec![plex_route(&[sha256(KEY)])]);
    let (ctx, at, _) = plex_ctx("plex-identity", Some("managed"), held, http.clone());
    let fillers = fillers(&at);
    let server = crate::app::targets::MediaServer::of(&fillers);
    let services = vec![media_server_svc(), requests_svc(), request_gate()];

    let admin = crate::seed::run::identity::seed_media_server_admin(&ctx, server.as_ref()).await;
    let (wirings, _) = crate::seed::run::identity::seed_request_identity(
        &ctx,
        &services,
        &fillers,
        &crate::baseline::Baseline::new(),
        server.as_ref(),
        admin,
    )
    .await;

    let states: Vec<(&str, &State)> = wirings
        .iter()
        .map(|wiring| (wiring.connection.as_str(), &wiring.state))
        .collect();
    assert!(
        states.contains(&(crate::seed::IDENTITY, &State::AlreadyWired)),
        "{states:?}"
    );
    assert!(
        states.iter().any(
            |(connection, state)| connection.ends_with("connection") && **state == State::Wired
        ),
        "{states:?}"
    );
    assert!(
        !states
            .iter()
            .any(|(connection, _)| connection.contains("administrator password")),
        "{states:?}"
    );
    assert_eq!(asked_with(&http, "link_media_server").len(), 1);
}

#[tokio::test]
async fn a_link_that_cannot_be_read_or_written_is_said_and_a_rehearsal_writes_none() {
    let elsewhere = crate::ports::service::Endpoint {
        host: "plex".to_owned(),
        port: 32400,
        base: String::new(),
    };
    let http = Fake::by_path_in_turn(vec![
        (
            intake_at("media_server_link"),
            vec![
                media::refusing(),
                media::answered(&serde_json::json!({"at": elsewhere, "key": ""})),
            ],
        ),
        (intake_at("link_media_server"), vec![media::refusing()]),
    ]);
    let (ctx, at, _) = plex_ctx("plex-link-refused", None, Gate::default(), http.clone());
    let Some(server) = server(&at) else {
        return;
    };
    let asker = server.served.clone();
    let mut rehearsing = ctx.clone();
    rehearsing.dry_run = true;
    let link = |ctx: Ctx| {
        let (http, server, asker) = (http.clone(), server.clone(), asker.clone());
        async move {
            crate::seed::run::plex_linking::linked(&ctx, &requests(&http), &asker, &server)
                .await
                .state
        }
    };

    let unread = link(ctx.clone()).await;
    let rehearsed = link(rehearsing).await;
    let unwritten = link(ctx).await;

    assert!(matches!(unread, State::Failed { .. }), "{unread:?}");
    assert!(
        matches!(rehearsed, State::WouldWire { yours: Some(_), .. }),
        "{rehearsed:?}"
    );
    assert!(matches!(unwritten, State::Failed { .. }), "{unwritten:?}");
    assert_eq!(asked_with(&http, "link_media_server").len(), 1);
}

/// The request service set up through a claimed Plex over `ctx`, its identity and link.
async fn identity_over(ctx: &Ctx, at: &std::path::Path) -> Vec<(String, State)> {
    let fillers = fillers(at);
    let server = crate::app::targets::MediaServer::of(&fillers);
    let services = vec![media_server_svc(), requests_svc(), request_gate()];
    let admin = crate::seed::run::identity::seed_media_server_admin(ctx, server.as_ref()).await;
    crate::seed::run::identity::seed_request_identity(
        ctx,
        &services,
        &fillers,
        &crate::baseline::Baseline::new(),
        server.as_ref(),
        admin,
    )
    .await
    .0
    .into_iter()
    .map(|wiring| (wiring.connection, wiring.state))
    .collect()
}

/// Plex and a request service set up where `initialized`, `establish` answered
/// `establishing`, telling nobody.
fn setting_up(initialized: bool, establishing: Answer) -> Arc<Fake> {
    plex(
        &[KEY],
        None,
        vec![
            (intake_at("establish"), vec![establishing]),
            (
                intake_at("initialized"),
                vec![media::answered(&initialized)],
            ),
            (
                intake_at("telling"),
                vec![media::answered(&serde_json::json!({
                    "enabled": false, "occasions": [], "others": false
                }))],
            ),
            (intake_at("tell"), vec![media::taken()]),
        ],
    )
}

#[tokio::test]
async fn a_rehearsal_says_the_request_service_would_be_pointed_at_the_plex_route() {
    let http = setting_up(false, media::taken());
    let held = Gate::admitting(vec![plex_route(&[sha256(KEY)])]);
    let (mut ctx, at, _) = plex_ctx(
        "plex-identity-rehearsed",
        Some("managed"),
        held,
        http.clone(),
    );
    ctx.dry_run = true;

    let wirings = identity_over(&ctx, &at).await;

    assert!(
        wirings
            .iter()
            .any(|(connection, state)| connection.ends_with("connection")
                && matches!(state, State::WouldWire { yours: None, .. })),
        "{wirings:?}"
    );
    assert!(asked_with(&http, "establish").is_empty());
}

#[tokio::test]
async fn owner_waiting_on_the_operators_sign_in_points_the_request_service_nowhere() {
    let http = setting_up(false, media::refusing());
    let held = Gate::admitting(vec![plex_route(&[])]);
    let (ctx, at, _) = plex_ctx("plex-identity-waiting", Some("owner"), held, http.clone());

    let wirings = identity_over(&ctx, &at).await;

    assert!(
        wirings
            .iter()
            .any(|(connection, state)| connection == crate::seed::IDENTITY
                && matches!(state, State::Skipped { .. })),
        "{wirings:?}"
    );
    assert!(!wirings
        .iter()
        .any(|(connection, _)| connection.ends_with("connection")));
}
