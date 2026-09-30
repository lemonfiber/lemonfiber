//! Jellyfin's cross-origin allow-list, held to the household front door's origin.

use super::*;

/// The origin the front door below is reached at.
const DOOR: &str = "http://192.168.1.20:5055";

/// A service published to the household.
fn published(mut service: lemonfiber_manifest::Service) -> lemonfiber_manifest::Service {
    service.bind = Some(lemonfiber_manifest::Bind::Lan);
    service
}

/// A stack with the media server and the request service, both on the household tier.
fn stack() -> Vec<lemonfiber_manifest::Service> {
    vec![published(jellyfin_svc()), published(seerr_svc())]
}

/// A context whose household is reached at a recorded address, with the media server's
/// administrator recorded where `administered` says, answering over `http`.
fn cors_ctx(name: &str, administered: bool, http: Arc<Fake>) -> Ctx {
    // Kept rather than scoped to this helper, because the context reads the file long
    // after the helper returns and a scratch directory goes when its handle does.
    let at = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(&at);
    let env = at.join(".env");
    let _ = std::fs::write(&env, "DATA_ROOT=/srv/media\n");
    if administered {
        let _ = store::set(
            &env,
            crate::config::JELLYFIN_ADMIN_PASSWORD_KEY,
            &lemonfiber_fixtures::support::a_password(),
        );
    }
    a_context()
        .environment(crate::platform::Environment::LinuxNative)
        .settings(Settings {
            env_file: Some(env),
            household_host: Some("192.168.1.20".to_owned()),
            ..Settings::default()
        })
        .build()
        .with_http(http)
}

/// A media server whose allow-list reads as `first`, then as `after` once written.
fn serving(first: &'static str, after: &'static str) -> Arc<Fake> {
    Fake::by_path_in_turn(vec![
        (
            "/Users/AuthenticateByName",
            vec![Answer::reply(200, r#"{"AccessToken":"token"}"#); 4],
        ),
        (
            "/System/Configuration",
            vec![
                Answer::reply(200, first),
                Answer::reply(200, first),
                Answer::reply(204, ""),
                Answer::reply(200, after),
            ],
        ),
    ])
}

/// The allow-list at Jellyfin's default, and once it names the front door alone.
const OPEN: &str = r#"{"CorsHosts":["*"]}"#;
const CLOSED: &str = r#"{"CorsHosts":["http://192.168.1.20:5055"]}"#;

/// The one wiring this pass reports, and the configurations it wrote.
async fn seeded_cors(ctx: &Ctx, http: &Fake) -> (Option<State>, Vec<String>) {
    let wiring = super::super::cors::seed_cors(ctx, &stack()).await;
    let written = http
        .requests()
        .into_iter()
        .filter(|asked| {
            asked.method == Method::Post && asked.url.ends_with("/System/Configuration")
        })
        .filter_map(|asked| asked.body)
        .collect();
    (wiring.map(|one| one.state), written)
}

/// An open list is closed to the front door's origin, and the pass says it wired it.
#[tokio::test]
async fn an_open_list_is_closed_to_the_front_door() {
    let http = serving(OPEN, CLOSED);
    let ctx = cors_ctx("cors-open", true, http.clone());

    let (state, written) = seeded_cors(&ctx, &http).await;

    assert_eq!(state, Some(State::Wired));
    assert_eq!(written, vec![format!(r#"{{"CorsHosts":["{DOOR}"]}}"#)]);
}

/// A list already naming the front door alone is left as it is.
#[tokio::test]
async fn a_list_naming_the_door_alone_is_left_alone() {
    let http = serving(CLOSED, CLOSED);
    let ctx = cors_ctx("cors-closed", true, http.clone());

    let (state, written) = seeded_cors(&ctx, &http).await;

    assert_eq!(state, Some(State::AlreadyWired));
    assert!(written.is_empty(), "{written:?}");
}

/// A list naming an address the front door has moved from follows it.
#[tokio::test]
async fn a_list_follows_the_front_door_when_it_moves() {
    let http = serving(r#"{"CorsHosts":["http://192.168.1.9:5055"]}"#, CLOSED);
    let ctx = cors_ctx("cors-moved", true, http.clone());

    let (state, written) = seeded_cors(&ctx, &http).await;

    assert_eq!(state, Some(State::Wired));
    assert_eq!(written.len(), 1, "{written:?}");
}

/// A rehearsal says what it would write over what is there, and writes nothing.
#[tokio::test]
async fn a_rehearsal_says_what_it_would_write_and_writes_nothing() {
    let http = serving(r#"{"CorsHosts":[]}"#, CLOSED);
    let mut ctx = cors_ctx("cors-rehearsed", true, http.clone());
    ctx.dry_run = true;

    let (state, written) = seeded_cors(&ctx, &http).await;

    assert_eq!(
        state,
        Some(State::WouldWire {
            yours: Some("every origin".to_owned()),
            ours: Some(DOOR.to_owned()),
        })
    );
    assert!(written.is_empty(), "{written:?}");
}

/// With no front door address the list is left as it stands and the pass warns,
/// because the empty list is the one that opens it.
#[tokio::test]
async fn with_no_front_door_address_nothing_is_written_and_the_pass_warns() {
    let http = serving(OPEN, CLOSED);
    let mut ctx = cors_ctx("cors-nowhere", true, http.clone());
    ctx.settings.household_host = None;

    let wiring = super::super::cors::seed_cors(&ctx, &stack()).await;

    assert!(
        wiring
            .as_ref()
            .is_some_and(|one| is_skipped(one) && one.severity.is_warning()),
        "{wiring:?}"
    );
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

/// A media server lemonfiber holds no account on is not configured; a rehearsal before
/// the identity step has minted one says what the run would write; and a stack with no
/// media server has nothing to hold.
#[tokio::test]
async fn without_a_credential_or_a_media_server_nothing_is_asked() {
    let http = serving(OPEN, CLOSED);
    let ctx = cors_ctx("cors-uncredentialled", false, http.clone());
    assert!(super::super::cors::seed_cors(&ctx, &stack())
        .await
        .is_none());

    let mut rehearsing = cors_ctx("cors-uncredentialled-rehearsed", false, http.clone());
    rehearsing.dry_run = true;
    let wiring = super::super::cors::seed_cors(&rehearsing, &stack()).await;
    assert_eq!(
        wiring.map(|one| one.state),
        Some(State::WouldWire {
            yours: None,
            ours: Some(DOOR.to_owned()),
        })
    );

    assert!(
        super::super::cors::seed_cors(&rehearsing, &[published(jellyfin_svc())])
            .await
            .is_none()
    );
    assert!(
        super::super::cors::seed_cors(&ctx, &[published(seerr_svc())])
            .await
            .is_none()
    );
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

/// A media server that cannot be read is reported rather than written over.
#[tokio::test]
async fn a_media_server_that_cannot_be_read_is_reported() {
    let http = Fake::silent();
    let ctx = cors_ctx("cors-silent", true, http.clone());

    let (state, written) = seeded_cors(&ctx, &http).await;

    assert!(
        matches!(state, Some(State::Skipped { .. } | State::Failed { .. })),
        "{state:?}"
    );
    assert!(written.is_empty(), "{written:?}");
}

/// A rehearsal over a list that names origins says which it would replace.
#[tokio::test]
async fn a_rehearsal_names_the_origins_it_would_replace() {
    let http = serving(r#"{"CorsHosts":["*","http://elsewhere:80"]}"#, CLOSED);
    let mut ctx = cors_ctx("cors-rehearsed-named", true, http.clone());
    ctx.dry_run = true;

    let (state, _) = seeded_cors(&ctx, &http).await;

    assert_eq!(
        state,
        Some(State::WouldWire {
            yours: Some("*, http://elsewhere:80".to_owned()),
            ours: Some(DOOR.to_owned()),
        })
    );
}

/// A write the media server refuses is reported rather than called done.
#[tokio::test]
async fn a_write_the_media_server_refuses_is_reported() {
    let http = Fake::by_path_in_turn(vec![
        (
            "/Users/AuthenticateByName",
            vec![Answer::reply(200, r#"{"AccessToken":"token"}"#); 3],
        ),
        (
            "/System/Configuration",
            vec![
                Answer::reply(200, OPEN),
                Answer::reply(200, OPEN),
                Answer::reply(400, "refused"),
            ],
        ),
    ]);
    let ctx = cors_ctx("cors-refused", true, http.clone());

    let (state, written) = seeded_cors(&ctx, &http).await;

    assert!(
        matches!(state, Some(State::Failed { .. } | State::Skipped { .. })),
        "{state:?}"
    );
    assert_eq!(written.len(), 1, "{written:?}");
}

/// A stack that publishes nothing to the household, and a door that publishes no port,
/// leave no origin to name, so nothing is written and the pass warns.
#[tokio::test]
async fn a_stack_with_no_door_or_a_door_with_no_port_names_no_origin() {
    let mut portless = published(seerr_svc());
    portless.port = None;
    for (name, services) in [
        ("cors-no-door", vec![jellyfin_svc(), seerr_svc()]),
        ("cors-no-port", vec![jellyfin_svc(), portless]),
    ] {
        let http = serving(OPEN, CLOSED);
        let ctx = cors_ctx(name, true, http.clone());

        let wiring = super::super::cors::seed_cors(&ctx, &services).await;

        assert!(
            wiring
                .as_ref()
                .is_some_and(|one| is_skipped(one) && one.severity.is_warning()),
            "{name}: {wiring:?}"
        );
        assert!(http.requests().is_empty(), "{name}: {:?}", http.requests());
    }
}
