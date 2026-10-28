use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::serving::Serves;

use super::{adapter, failed_at, json_at, posted, read_through, source, FRONTED};
use crate::ports::http::{Http, Request, Response, Unreachable};

fn serving_media() -> String {
    FRONTED
        .replace(
            "speaks      = [\"subtitles.fetch@1\"]",
            "speaks      = [\"media.serve@1\"]",
        )
        .replace(
            "provides    = [\"subtitles.fetch\"]",
            "provides    = [\"media.serve\"]",
        )
}

/// An adapter answering `serve_tls` as one whose upstream serves the certificate only once
/// it restarts, where it is handed the path the upstream's container holds it at and the
/// upstream's copy holds an internal certificate that reads, and everything else as `fake`
/// does.
struct Serving {
    fake: Arc<Fake>,
    serving: Arc<Serves>,
}

#[async_trait]
impl Http for Serving {
    async fn send(&self, request: &Request) -> Result<Response, Unreachable> {
        if !request.url.ends_with("/serve_tls") {
            return self.fake.send(request).await;
        }
        let held = self
            .serving
            .asked()
            .first()
            .and_then(|upstream| {
                upstream
                    .binds
                    .iter()
                    .find(|bound| bound.target == "/config")
            })
            .and_then(|bound| {
                crate::certificate::internal::kept(&bound.source)
                    .ok()
                    .flatten()
            });
        let handed = request
            .body
            .as_deref()
            .and_then(|body| serde_json::from_str::<serde_json::Value>(body).ok())
            .and_then(|asked| asked.get("path").cloned());
        let answered =
            held.is_some() && handed == Some(serde_json::json!("/config/lemonfiber/internal.p12"));
        Ok(Response {
            status: if answered { 205 } else { 500 },
            headers: Vec::new(),
            body: String::new(),
        })
    }
}

#[tokio::test]
async fn a_media_server_is_handed_a_throwaway_internal_certificate_where_its_container_holds_it() {
    let at = source("fronted-internal", &serving_media());
    let serving = Arc::new(Serves::on(49153).besides(Ok(adapter(Some(49154)))));
    let json = |body: &str| Answer::served(200, "application/json", body);
    let fake = Fake::by_path_in_turn(vec![
        ("/health", vec![Answer::reply(200, "ok")]),
        (
            "/lemonfiber/adapter/v1/ready",
            vec![Answer::reply(200, "{}")],
        ),
        (
            "/System/Info/Public",
            vec![json("{\"ProductName\": \"Jellyfin Server\"}")],
        ),
        (
            "/mint",
            vec![Answer::served(
                500,
                crate::ports::http::PROBLEM,
                r#"{"type": "unsupported", "detail": "it mints no keys"}"#,
            )],
        ),
        ("/lemonfiber/media.serve/v1/", vec![json("null")]),
    ]);
    let http = Arc::new(Serving {
        fake: Arc::clone(&fake),
        serving: Arc::clone(&serving),
    });
    let read = read_through(&at, &serving, http, Some(7)).await;
    let Ok(read) = read else {
        unreachable!("the plugin reads: {read:?}");
    };
    assert_eq!(failed_at(&read), Vec::new());

    let asked = posted(&fake, "/serve_tls");
    assert!(
        asked.is_empty(),
        "serve_tls is answered by the upstream's copy alone"
    );
    let recorded = json_at(&at, "conformance/media.serve@1/serve_tls-answers.json");
    assert_eq!(
        recorded.pointer("/response/status"),
        Some(&serde_json::json!(205)),
        "{recorded}"
    );
}

#[test]
fn only_an_adapter_speaking_media_serve_has_its_upstream_handed_one() {
    let manifest = lemonfiber_plugin::Manifest::from_toml(FRONTED).ok();
    let installed = manifest.map(|read| crate::plugin::installed::Installed::of(&read));
    let copy =
        crate::plugin::program::Throwaway::of(None, &lemonfiber_fixtures::ports::Chance::cycling());
    let (Some(installed), Ok(copy)) = (installed, copy) else {
        unreachable!("the fixture places its services and a copy is made");
    };
    let (Some(upstream), Some(subtitling)) =
        (installed.services.first(), installed.services.get(1))
    else {
        unreachable!("the fixture places an upstream and its adapter");
    };
    assert_eq!(
        super::super::super::served::internal(upstream, subtitling, &copy),
        Ok(None)
    );
    assert_eq!(crate::certificate::internal::kept(copy.path()), Ok(None));
    let mut serving = subtitling.clone();
    serving.speaks = vec!["media.serve@1".to_owned()];
    let handed = super::super::super::served::internal(upstream, &serving, &copy);
    let kept = crate::certificate::internal::kept(copy.path())
        .ok()
        .flatten()
        .map(|kept| kept.fingerprint);
    assert_eq!(
        handed,
        Ok(kept.map(|pin| ("/config/lemonfiber/internal.p12".to_owned(), pin)))
    );
}
