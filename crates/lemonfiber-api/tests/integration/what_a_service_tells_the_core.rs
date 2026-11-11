//! A service tells the core what happened only with the key the core minted for it.

use lemonfiber_api::guard::Arrived;
use lemonfiber_core::ports::digest::sha256;
use tower::ServiceExt as _;

use crate::door::*;
use crate::what_a_key_admits::{HERE, NEXT_DOOR};

/// The key the core handed the curator in these cases.
const KEY: &str = "a-key-minted-for-sonarr-alone";

/// What the curator says happened.
const FAILED: &str =
    r#"{"event":"failed","reference":"42","name":"Severance","reason":"no release"}"#;

/// A surface whose curator was handed [`KEY`].
fn a_machine(named: &str) -> axum::Router {
    let admission = keeping(&format!("events-{named}"));
    let mut ctx = world(Some(admission.clone()), Chance::exactly(None));
    let env = a_directory(&format!("events-env-{named}")).join(".env");
    if let Some(dir) = env.parent() {
        let _ = fs::create_dir_all(dir);
        let kept = serde_json::json!({
            "schema": 1,
            "written_by": "0.1.0",
            "keys": { "sonarr": sha256(KEY) },
        });
        let _ = fs::write(dir.join("notify-events.json"), kept.to_string());
    }
    ctx.settings.env_file = Some(env);
    let admitting = Arc::new(Admitting {
        kept: Some(admission),
        ..Admitting::default()
    });
    surface(ctx, &admitting).0
}

/// What the surface answers `body` sent with `key`, from `from`.
async fn told(
    router: axum::Router,
    key: Option<&str>,
    from: (IpAddr, bool),
    body: &str,
) -> (StatusCode, String) {
    sent(
        router,
        ("/api/notify/events", &from_here()),
        key,
        from,
        body,
    )
    .await
}

/// What the surface answers `body` sent to `path` carrying `headers` and `key`, from `from`.
async fn sent(
    router: axum::Router,
    (path, headers): (&str, &[(&'static str, String)]),
    key: Option<&str>,
    from: (IpAddr, bool),
    body: &str,
) -> (StatusCode, String) {
    let mut building = Request::builder()
        .method("POST")
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");
    for (name, value) in headers {
        building = building.header(*name, value);
    }
    if let Some(key) = key {
        building = building.header(header::AUTHORIZATION, format!("Bearer {key}"));
    }
    let Ok(mut request) = building.body(Body::from(body.to_owned())) else {
        unreachable!("the request a test writes is one that can be built")
    };
    request.extensions_mut().insert(Arrived {
        from: from.0,
        encrypted: from.1,
    });
    let Ok(response) = router.oneshot(request).await;
    let status = response.status();
    let read = to_bytes(response.into_body(), 64 * 1024)
        .await
        .unwrap_or_default();
    (status, String::from_utf8_lossy(&read).into_owned())
}

#[tokio::test]
async fn an_event_is_taken_with_the_key_its_service_was_handed_and_nothing_else() {
    let router = a_machine("taken");

    let (taken, _) = told(router.clone(), Some(KEY), (HERE, false), FAILED).await;
    let (unkeyed, said) = told(router.clone(), None, (HERE, false), FAILED).await;
    let (wrong, _) = told(router.clone(), Some("guessed"), (HERE, false), FAILED).await;

    assert_eq!(taken, StatusCode::NO_CONTENT);
    assert_eq!(unkeyed, StatusCode::UNAUTHORIZED, "{said}");
    assert!(said.contains("NOTIFY-10"), "{said}");
    assert_eq!(wrong, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_key_sent_in_the_clear_from_elsewhere_or_with_no_event_is_refused() {
    let router = a_machine("refused");

    let (clear, _) = told(router.clone(), Some(KEY), (NEXT_DOOR, false), FAILED).await;
    let (shapeless, _) = told(router.clone(), Some(KEY), (HERE, false), "{}").await;

    assert_ne!(clear, StatusCode::NO_CONTENT);
    assert_eq!(shapeless, StatusCode::BAD_REQUEST);
}

/// A container on this machine, as the engine's bridge network names it.
const CONTAINER: IpAddr = IpAddr::V4(std::net::Ipv4Addr::new(172, 17, 0, 5));

/// What a container reaching this machine by name says it is reaching.
fn from_a_container() -> Vec<(&'static str, String)> {
    vec![(
        "host",
        format!("{}:{PORT}", lemonfiber_core::notify::events::HOST),
    )]
}

#[tokio::test]
async fn a_container_naming_this_machine_as_containers_do_tells_the_core_over_tls() {
    let router = a_machine("container");
    let named = from_a_container();

    let (taken, said) = sent(
        router.clone(),
        ("/api/notify/events", &named),
        Some(KEY),
        (CONTAINER, true),
        FAILED,
    )
    .await;
    let (clear, _) = sent(
        router,
        ("/api/notify/events", &named),
        Some(KEY),
        (CONTAINER, false),
        FAILED,
    )
    .await;

    assert_eq!(taken, StatusCode::NO_CONTENT, "{said}");
    assert_ne!(clear, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn only_the_events_door_answers_the_name_containers_reach_this_machine_by() {
    let router = a_machine("container-elsewhere");
    let name = format!("{}:{PORT}", lemonfiber_core::notify::events::HOST);
    let paged = vec![
        ("host", name.clone()),
        ("origin", format!("https://{name}")),
    ];

    let (elsewhere, _) = sent(
        router.clone(),
        ("/api/actions/stack-up", &from_a_container()),
        Some(KEY),
        (CONTAINER, true),
        "{}",
    )
    .await;
    let (from_a_page, _) = sent(
        router,
        ("/api/notify/events", &paged),
        Some(KEY),
        (CONTAINER, true),
        FAILED,
    )
    .await;

    assert_ne!(elsewhere, StatusCode::NO_CONTENT);
    assert_ne!(elsewhere, StatusCode::OK);
    assert_ne!(from_a_page, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn under_the_name_containers_use_an_event_still_needs_its_services_key() {
    let router = a_machine("container-keyless");
    let named = from_a_container();

    let (unkeyed, said) = sent(
        router.clone(),
        ("/api/notify/events", &named),
        None,
        (CONTAINER, true),
        FAILED,
    )
    .await;
    let (guessed, _) = sent(
        router,
        ("/api/notify/events", &named),
        Some("a-key-nobody-was-handed"),
        (CONTAINER, true),
        FAILED,
    )
    .await;

    assert_eq!(unkeyed, StatusCode::UNAUTHORIZED, "{said}");
    assert!(said.contains("NOTIFY-10"), "{said}");
    assert_eq!(guessed, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn the_name_containers_use_opens_that_one_path_for_that_one_method() {
    let router = a_machine("container-exact");
    let named = from_a_container();
    let asked = |method: &'static str, path: &'static str| {
        let router = router.clone();
        let named = named.clone();
        async move {
            let mut building = Request::builder()
                .method(method)
                .uri(path)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {KEY}"));
            for (name, value) in &named {
                building = building.header(*name, value);
            }
            let Ok(mut request) = building.body(Body::from(FAILED)) else {
                unreachable!("the request a test writes is one that can be built")
            };
            request.extensions_mut().insert(Arrived {
                from: CONTAINER,
                encrypted: true,
            });
            let Ok(response) = router.oneshot(request).await;
            response.status()
        }
    };

    let fetched = asked("GET", "/api/notify/events").await;
    let beneath = asked("POST", "/api/notify/events/more").await;
    let beside = asked("POST", "/api/notify/eventsmore").await;

    for status in [fetched, beneath, beside] {
        assert!(status.is_client_error(), "{status}");
    }
}

#[tokio::test]
async fn an_event_meets_the_same_limits_on_its_size_and_on_guessing_as_any_request() {
    let router = a_machine("container-limits");
    let named = from_a_container();
    let oversized = format!(
        r#"{{"event":"health","name":"{}"}}"#,
        "x".repeat(3 * 1024 * 1024)
    );

    let (large, _) = sent(
        router.clone(),
        ("/api/notify/events", &named),
        Some(KEY),
        (CONTAINER, true),
        &oversized,
    )
    .await;
    let mut last = StatusCode::OK;
    for _ in 0..8 {
        (last, _) = sent(
            router.clone(),
            ("/api/notify/events", &named),
            Some("a-guess"),
            (CONTAINER, true),
            FAILED,
        )
        .await;
    }

    assert!(large.is_client_error(), "{large}");
    assert_eq!(last, StatusCode::TOO_MANY_REQUESTS);
}

#[test]
fn the_door_is_where_the_core_tells_each_service_to_send_what_happened() {
    assert_eq!(
        lemonfiber_api::notify_events::EVENTS,
        lemonfiber_core::notify::events::PATH
    );
}
