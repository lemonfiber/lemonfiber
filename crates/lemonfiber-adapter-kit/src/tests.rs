use std::sync::Arc;

use async_trait::async_trait;
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use lemonfiber_contract::adapter::{About, Release, KEY_FILE};
use lemonfiber_contract::capabilities::subtitles::fetch;
use lemonfiber_ports::media::Kind;
use lemonfiber_ports::service::{Failure, Subtitles, Watched, Watching};
use tower::ServiceExt as _;

use super::{key_in, serve, serving, Kit};

const KEY: &str = "the-plugins-key";

/// A subtitle finder watching nothing.
struct Finding;

#[async_trait]
impl Subtitles for Finding {
    async fn watching(&self, _which: Kind) -> Result<Watching, Failure> {
        Ok(Watching {
            enabled: false,
            host: String::new(),
            port: 0,
            keyed: false,
        })
    }
    async fn watch(&self, _watched: &Watched) -> Result<(), Failure> {
        Ok(())
    }
}

fn about() -> About {
    About {
        speaks: vec!["subtitles.fetch@1".to_owned()],
        upstream: "a solver".to_owned(),
        releases: vec![Release {
            version: "3.4".to_owned(),
            digest: "sha256:abc".to_owned(),
        }],
    }
}

fn kit() -> Kit {
    Kit::new(KEY, about()).serving(fetch::served(Arc::new(Finding)))
}

/// What the kit answers `method` at `path`, carrying `key`, as its status, its type and
/// its body.
async fn asked(
    kit: Kit,
    method: &str,
    path: &str,
    key: Option<&str>,
) -> (StatusCode, String, String) {
    let mut request = Request::builder().method(method).uri(path);
    if let Some(key) = key {
        request = request.header("authorization", format!("Bearer {key}"));
    }
    let response = kit
        .router()
        .oneshot(
            request
                .body(Body::from(r#"{"which":"tv"}"#))
                .unwrap_or_default(),
        )
        .await;
    let Ok(response) = response;
    let status = response.status();
    let sort = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default();
    (status, sort, body)
}

#[tokio::test]
async fn an_operation_is_served_under_the_plugins_key() {
    let (status, sort, body) = asked(
        kit(),
        "POST",
        "/lemonfiber/subtitles.fetch/v1/watching",
        Some(KEY),
    )
    .await;
    assert_eq!(
        (status, sort.as_str()),
        (StatusCode::OK, "application/json")
    );
    assert_eq!(
        body,
        r#"{"enabled":false,"host":"","port":0,"keyed":false}"#
    );
}

#[tokio::test]
async fn nothing_is_answered_without_the_plugins_key() {
    for key in [None, Some(""), Some("the-plugins-kez"), Some("short")] {
        for (method, path) in [
            ("POST", "/lemonfiber/subtitles.fetch/v1/watching"),
            ("GET", "/lemonfiber/adapter/v1/about"),
            ("GET", "/lemonfiber/adapter/v1/ready"),
        ] {
            let (status, _, body) = asked(kit(), method, path, key).await;
            assert_eq!(
                (status, body.as_str()),
                (StatusCode::UNAUTHORIZED, ""),
                "{key:?} {path}"
            );
        }
    }
    let (status, _, _) = asked(
        Kit::new("", about()),
        "GET",
        "/lemonfiber/adapter/v1/about",
        Some(""),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn an_operation_nothing_serves_is_refused_as_a_problem() {
    for path in [
        "/lemonfiber/subtitles.fetch/v1/unheard",
        "/lemonfiber/subtitles.fetch/v2/watching",
        "/lemonfiber/media.serve/v1/title",
    ] {
        let (status, sort, body) = asked(kit(), "POST", path, Some(KEY)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(sort, "application/problem+json", "{path}");
        assert!(body.contains("unknown-operation"), "{path}: {body}");
    }
}

#[tokio::test]
async fn the_adapter_says_what_it_is_and_whether_its_upstream_answers() {
    let (status, _, body) = asked(kit(), "GET", "/lemonfiber/adapter/v1/about", Some(KEY)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(serde_json::from_str::<About>(&body).ok(), Some(about()));
    let (_, _, ready) = asked(kit(), "GET", "/lemonfiber/adapter/v1/ready", Some(KEY)).await;
    assert_eq!(ready, "true");
    let down = kit().ready_when(Arc::new(|| Box::pin(async { false })));
    let (_, _, ready) = asked(down, "GET", "/lemonfiber/adapter/v1/ready", Some(KEY)).await;
    assert_eq!(ready, "false");
}

#[test]
fn the_key_is_read_from_where_the_core_wrote_it() {
    let dir = std::env::temp_dir().join(format!("adapter-kit-key-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join(KEY_FILE), format!("{KEY}\n"));
    assert_eq!(key_in(&dir).ok().as_deref(), Some(KEY));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(key_in(&dir).is_err());
}

#[tokio::test]
async fn the_cores_client_asks_a_served_adapter_over_the_wire() {
    let Ok(listener) = tokio::net::TcpListener::bind("127.0.0.1:0").await else {
        unreachable!("a loopback port is always free to bind");
    };
    let at = listener
        .local_addr()
        .map(|address| address.to_string())
        .unwrap_or_default();
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(serving(kit(), listener, async {
        let _ = stopped.await;
    }));
    let client = |key: &str| {
        fetch::Adapter(lemonfiber_contract::Contracted::new(
            Arc::new(lemonfiber_adapters::Web::new()),
            format!("http://{at}"),
            "subtitle-adapter",
            key,
        ))
    };
    let watching = client(KEY).watching(Kind::Tv).await;
    let refused = client("not-the-key").watching(Kind::Tv).await;
    let _ = stop.send(());
    assert!(matches!(server.await, Ok(Ok(()))));
    assert_eq!(watching.ok().map(|held| held.enabled), Some(false));
    assert!(matches!(refused, Err(Failure::Unauthorised { .. })));
}

#[tokio::test]
async fn an_address_is_served_until_the_process_ends_and_one_that_cannot_be_bound_is_said() {
    let served = tokio::spawn(serve(kit(), "127.0.0.1:0"));
    tokio::task::yield_now().await;
    assert!(!served.is_finished());
    served.abort();
    assert!(serve(kit(), "not an address").await.is_err());
}
