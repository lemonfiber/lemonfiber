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
    let mut building = Request::builder()
        .method("POST")
        .uri("/api/notify/events")
        .header(header::CONTENT_TYPE, "application/json");
    for (name, value) in from_here() {
        building = building.header(name, value);
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
