use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::serving::Serves;
use serde_json::json;

use super::{failed_at, json_at, posted, read_through, recorded, source};
use crate::plugin::recording::Took;
use crate::ports::http::{Http, Request, Response, Unreachable};

const GUARD: &str = r#"
schema_version = 1

[plugin]
id          = "guard"
name        = "Guard"
version     = "1.0.0"
description = "Lets the household's players in, and nothing else"
without_it  = "The media server answers whoever reaches it"
upstream    = "https://github.com/lemonfiber/lemonfiber-door"
license     = "MIT"
forms       = ["library", "full"]

[[service]]
id          = "guard"
name        = "Guard"
image       = "ghcr.io/lemonfiber/guard"
digest      = "sha256:1111111111111111111111111111111111111111111111111111111111111111"
tag         = "1.0.0"
port        = 8097
bind        = "lan"
listens     = 8096
contract    = 8081
health      = { kind = "http", path = "/health", timeout_s = 2 }
criticality = "important"
reaches     = ""
asks_for    = "Nothing beyond this machine."
config_path = "/config"
user_mode   = "uid"
speaks      = ["stream.guard@1"]
provides    = ["stream.guard"]

[[proof]]
id      = "guard.refuses"
title   = "The door refuses a stream it was not asked about"
fixture = "fixtures/refused.json"
why     = "Nothing reaches the media server's bytes without the door asking."
request = { method = "GET", path = "/Videos/unknown/stream" }
expect  = { status = 403, json = { error = "refused" } }
"#;

const PINNED: &str =
    "ghcr.io/lemonfiber/guard@sha256:1111111111111111111111111111111111111111111111111111111111111111";

#[tokio::test]
async fn a_speaker_fronting_nothing_is_its_own_upstream_enveloped_before_its_proofs() {
    let at = source("alone-guard", GUARD);
    let serving = Serves::on(49153).contracted(49155);
    let http = Fake::by_path_in_turn(vec![
        ("/health", vec![Answer::reply(200, "ok")]),
        (
            "/lemonfiber/adapter/v1/ready",
            vec![Answer::reply(200, "{}")],
        ),
        (
            "/Videos/unknown/stream",
            vec![Answer::served(
                403,
                "application/json",
                "{\"error\": \"refused\"}",
            )],
        ),
        (
            "/guarding",
            vec![Answer::served(200, "application/json", "null")],
        ),
        (
            "/guard",
            vec![
                Answer::reply(204, ""),
                Answer::served(401, "text/plain", "Unauthorized"),
                Answer::reply(204, ""),
            ],
        ),
        (
            "/certificate",
            vec![Answer::served(
                200,
                "application/json",
                "{\"fingerprint\": \"ab\", \"certificate\": \"cd\"}",
            )],
        ),
    ]);
    let read = recorded(&at, &serving, &http, Some(7)).await;
    assert_eq!(failed_at(&read), Vec::new());
    assert!(
        read.recordings
            .iter()
            .all(|taken| taken.took == Took::Unnoted),
        "{:?}",
        read.recordings
    );
    let served = serving.asked();
    assert_eq!(served.len(), 1, "one container is its own upstream");
    assert_eq!(
        served
            .first()
            .map(|one| (one.image.as_str(), one.listens, one.contract)),
        Some((PINNED, Some(8096), Some(8081)))
    );
    let urls: Vec<String> = http.requests().into_iter().map(|one| one.url).collect();
    let enveloped = urls.iter().position(|url| url.ends_with("/guard"));
    let proved = urls
        .iter()
        .position(|url| url.ends_with("/Videos/unknown/stream"));
    assert!(
        enveloped.is_some() && enveloped < proved,
        "the envelope is applied before the proof is asked: {urls:?}"
    );
    assert!(urls
        .iter()
        .filter(|url| url.contains("/lemonfiber/"))
        .all(|url| url.starts_with("http://127.0.0.1:49155/")));
    assert!(proved
        .and_then(|at| urls.get(at))
        .is_some_and(|url| url.starts_with("http://127.0.0.1:49153/")));
    let envelope = posted(&http, "/guard")
        .first()
        .and_then(|request| request.body.clone())
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .unwrap_or_default();
    assert_eq!(
        envelope.pointer("/guarded/upstream"),
        Some(&json!("http://recording-media-server:58082"))
    );
    assert!(envelope
        .pointer("/guarded/declared/bytes")
        .is_some_and(|bytes| bytes.to_string().contains("Videos")));
    let stood: Vec<u16> = serving.stood_in().iter().map(|one| one.port).collect();
    assert_eq!(stood, vec![58_082, 58_081]);
    assert_eq!(
        json_at(&at, "fixtures/refused.json").pointer("/recorded_from"),
        Some(&json!(PINNED))
    );
    assert_eq!(
        json_at(&at, "conformance/stream.guard@1/certificate-answers.json")
            .pointer("/recorded_from"),
        Some(&json!(PINNED))
    );
    assert_eq!(serving.removed().len(), 1);
}

#[tokio::test]
async fn a_guard_refusing_its_envelope_or_without_its_stand_ins_records_nothing() {
    let refusing = Fake::by_path_in_turn(vec![
        ("/health", vec![Answer::reply(200, "ok")]),
        (
            "/lemonfiber/adapter/v1/ready",
            vec![Answer::reply(200, "{}")],
        ),
        (
            "/guard",
            vec![Answer::served(
                500,
                crate::ports::http::PROBLEM,
                r#"{"type": "refused", "detail": "no envelope today"}"#,
            )],
        ),
    ]);
    let serving = Serves::on(49153).contracted(49155);
    let read = recorded(
        &source("alone-refused", GUARD),
        &serving,
        &refusing,
        Some(7),
    )
    .await;
    assert!(
        failed_at(&read)
            .iter()
            .any(|(step, why)| step == "envelope" && why.contains("no envelope today")),
        "{:?}",
        failed_at(&read)
    );

    let unanswering = Serves::on(49153).contracted(49155).unanswering(
        crate::ports::program::Unran::Unreachable {
            reason: "no port to answer on".to_owned(),
        },
    );
    let read = recorded(
        &source("alone-unanswering", GUARD),
        &unanswering,
        &refusing,
        Some(7),
    )
    .await;
    assert!(
        failed_at(&read)
            .iter()
            .any(|(step, _)| step == "recording-media-server"),
        "{:?}",
        failed_at(&read)
    );
    assert!(unanswering.asked().is_empty(), "nothing is started");
}

/// A guard answering its certificate with the fingerprint of the one it was served with,
/// and everything else as `fake` does.
struct Certifying {
    fake: Arc<Fake>,
    serving: Arc<Serves>,
}

#[async_trait]
impl Http for Certifying {
    async fn send(&self, request: &Request) -> Result<Response, Unreachable> {
        if !request.url.ends_with("/certificate") {
            return self.fake.send(request).await;
        }
        let fingerprint = self
            .serving
            .asked()
            .first()
            .and_then(|served| served.binds.iter().find(|bound| bound.target == "/config"))
            .and_then(|bound| crate::certificate::kept(&bound.source).ok().flatten())
            .map(|kept| kept.fingerprint)
            .unwrap_or_default();
        Ok(Response {
            status: 200,
            headers: vec![("Content-Type".to_owned(), "application/json".to_owned())],
            body: json!({ "fingerprint": fingerprint, "certificate": "cd" }).to_string(),
        })
    }
}

#[tokio::test]
async fn the_fingerprint_of_the_certificate_a_run_made_is_scrubbed_so_a_rerun_reproduces_it() {
    let mut taken = Vec::new();
    for run in ["alone-certified-once", "alone-certified-again"] {
        let at = source(run, GUARD);
        let serving = Arc::new(Serves::on(49153).contracted(49155));
        let fake = Fake::by_path(vec![
            ("/health", Answer::reply(200, "ok")),
            ("/lemonfiber/adapter/v1/ready", Answer::reply(200, "{}")),
            (
                "/Videos/unknown/stream",
                Answer::served(403, "application/json", "{\"error\": \"refused\"}"),
            ),
            ("/guarding", Answer::served(200, "application/json", "null")),
            ("/guard", Answer::reply(204, "")),
        ]);
        let http = Arc::new(Certifying {
            fake,
            serving: Arc::clone(&serving),
        });
        let read = read_through(&at, &serving, http, Some(7)).await;
        assert!(read.is_ok(), "{read:?}");
        taken.push(json_at(
            &at,
            "conformance/stream.guard@1/certificate-answers.json",
        ));
    }
    let fingerprints: Vec<_> = taken
        .iter()
        .map(|recorded| recorded.pointer("/response/json/fingerprint").cloned())
        .collect();
    assert_eq!(
        fingerprints,
        vec![
            Some(json!(crate::plugin::recording::scrubbed::REDACTED)),
            Some(json!(crate::plugin::recording::scrubbed::REDACTED))
        ]
    );
    assert_eq!(taken.first(), taken.get(1));
}
