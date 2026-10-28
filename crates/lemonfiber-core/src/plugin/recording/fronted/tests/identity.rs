use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use serde_json::json;

use super::super::unfinished;
use super::{body, engine, failed_at, json_at, posted, recorded, source, FRONTED, MINTED};
use crate::ports::http::PROBLEM;

fn identifying() -> String {
    FRONTED
        .replace(
            "speaks      = [\"subtitles.fetch@1\"]",
            "speaks      = [\"identity.source@1\"]",
        )
        .replace(
            "provides    = [\"subtitles.fetch\"]",
            "provides    = [\"identity.source\"]",
        )
}

fn identity(startup: Vec<Answer>, created: Vec<Answer>) -> Arc<Fake> {
    identity_and(startup, created, Vec::new())
}

fn identity_and(
    startup: Vec<Answer>,
    created: Vec<Answer>,
    more: Vec<(&'static str, Vec<Answer>)>,
) -> Arc<Fake> {
    let json = |body: &str| Answer::served(200, "application/json", body);
    Fake::by_path_in_turn(
        [
            more,
            vec![
                ("/health", vec![Answer::reply(200, "ok")]),
                (
                    "/lemonfiber/adapter/v1/ready",
                    vec![Answer::reply(200, "true")],
                ),
                ("/System/Info/Public", vec![json("{}")]),
                ("/startup_completed", startup),
                ("/create_admin", created),
                ("/household", vec![json("[{\"id\": \"m1\"}]")]),
                (
                    "/whoever",
                    vec![json("{\"id\": \"m1\", \"token\": \"t0k3n\"}")],
                ),
                ("/invite", vec![json("{\"id\": \"m2\"}")]),
                ("/lemonfiber/identity.source/v1/", vec![json("null")]),
            ],
        ]
        .concat(),
    )
}

#[tokio::test]
async fn an_adapter_speaking_identity_is_first_run_through_and_asked_with_what_it_left() {
    let at = source("fronted-identity", &identifying());
    let json = |body: &str| Answer::served(200, "application/json", body);
    let http = identity(
        vec![
            json("false"),
            Answer::served(401, "text/plain", "Unauthorized"),
        ],
        vec![json("null")],
    );
    let read = recorded(&at, &engine(), &http, Some(7)).await;
    assert_eq!(failed_at(&read), Vec::new());
    assert_eq!(read.recordings.len(), 21);
    assert_eq!(posted(&http, "/create_admin").len(), 1);
    assert_eq!(posted(&http, "/startup_completed").len(), 2);
    assert_eq!(
        json_at(
            &at,
            "conformance/identity.source@1/startup_completed-answers.json"
        )
        .pointer("/response/json"),
        Some(&json!(false))
    );
    assert_eq!(
        json_at(
            &at,
            "conformance/identity.source@1/create_admin-answers.json"
        )
        .pointer("/response/status"),
        Some(&json!(200))
    );
    let asked = |fragment: &str| posted(&http, fragment).last().map(body);
    assert_eq!(
        asked("/create_admin"),
        Some(json!({"name": "admin", "password": MINTED}))
    );
    assert_eq!(
        asked("/replace_admin_password"),
        Some(json!({"replacement": MINTED}))
    );
    assert_eq!(
        asked("/whoever"),
        Some(json!({"device": "conformance-whoever", "name": "admin", "password": MINTED}))
    );
    assert_eq!(
        asked("/standing"),
        Some(json!({"signed": {"id": "m1", "token": "t0k3n"}}))
    );
    assert_eq!(asked("/sessions"), Some(json!({"member": "m1"})));
    assert_eq!(
        json_at(&at, "conformance/identity.source@1/whoever-answers.json")
            .pointer("/response/json"),
        Some(&json!({"id": "m1", "token": "<redacted>"}))
    );
}

fn unavailable() -> Answer {
    Answer::served(
        503,
        PROBLEM,
        r#"{"type": "unavailable", "detail": "jellyfin is not answering"}"#,
    )
}

#[tokio::test(start_paused = true)]
async fn a_first_run_waits_while_the_upstream_is_still_starting() {
    let at = source("fronted-identity-starting", &identifying());
    let json = |body: &str| Answer::served(200, "application/json", body);
    let http = identity(
        vec![
            json("false"),
            json("false"),
            json("false"),
            Answer::served(401, "text/plain", "Unauthorized"),
        ],
        vec![unavailable(), unavailable(), json("null")],
    );
    let read = recorded(&at, &engine(), &http, Some(7)).await;

    assert_eq!(failed_at(&read), Vec::new());
    assert_eq!(read.recordings.len(), 21);
    assert_eq!(posted(&http, "/create_admin").len(), 3);
    assert_eq!(
        json_at(
            &at,
            "conformance/identity.source@1/create_admin-answers.json"
        )
        .pointer("/response/status"),
        Some(&json!(200))
    );
}

#[tokio::test(start_paused = true)]
async fn a_first_run_still_not_answered_at_its_limit_fails_the_service_at_it() {
    let at = source("fronted-identity-never-up", &identifying());
    let json = |body: &str| Answer::served(200, "application/json", body);
    let http = identity(vec![json("false")], vec![unavailable()]);
    let read = recorded(&at, &engine(), &http, Some(7)).await;

    assert_eq!(
        failed_at(&read),
        vec![(
            "first run".to_owned(),
            crate::seed::NOT_ANSWERING.to_owned()
        )]
    );
    assert!(posted(&http, "/create_admin").len() > 1);
}

#[tokio::test]
async fn a_first_run_that_does_not_finish_fails_the_service_at_it() {
    let json = |body: &str| Answer::served(200, "application/json", body);
    for (startup, created, why) in [
        (json("true"), json("null"), "set up outside lemonfiber"),
        (
            json("false"),
            Answer::served(500, "text/plain", "broken"),
            "500 is not an answer the contract declares",
        ),
    ] {
        let at = source("fronted-unfinished", &identifying());
        let http = identity(vec![startup], vec![created]);
        let read = recorded(&at, &engine(), &http, Some(7)).await;
        let failed = failed_at(&read);
        assert!(
            matches!(failed.first(), Some((step, said)) if step == "first run" && said.contains(why)),
            "{failed:?}"
        );
        assert_eq!(posted(&http, "/startup_completed").len(), 1);
    }
    assert_eq!(unfinished(crate::seed::State::Wired), "Wired");
}

#[tokio::test]
async fn a_media_server_listed_before_its_identity_source_is_asked_after_it_its_token_scrubbed() {
    let manifest = identifying()
        .replace(
            "speaks      = [\"identity.source@1\"]",
            "speaks      = [\"media.serve@1\", \"identity.source@1\"]",
        )
        .replace(
            "provides    = [\"identity.source\"]",
            "provides    = [\"media.serve\", \"identity.source\"]",
        );
    let at = source("fronted-media-after-identity", &manifest);
    let json = |body: &str| Answer::served(200, "application/json", body);
    let http = identity_and(
        vec![
            json("false"),
            Answer::served(401, "text/plain", "Unauthorized"),
        ],
        vec![json("null")],
        vec![
            ("/signed_in", vec![json("\"5e55i0n-issued-by-the-server\"")]),
            ("/mint", vec![json("\"k3y-issued-by-the-server\"")]),
            ("/lemonfiber/media.serve/v1/", vec![json("null")]),
        ],
    );
    let read = recorded(&at, &engine(), &http, Some(7)).await;

    assert_eq!(failed_at(&read), Vec::new());
    let urls: Vec<String> = http.requests().into_iter().map(|asked| asked.url).collect();
    let first = |fragment: &str| urls.iter().position(|url| url.ends_with(fragment));
    let order = (first("/household"), first("/signed_in"));
    assert!(
        matches!(order, (Some(read), Some(reading)) if read < reading),
        "{urls:?}"
    );
    for issued in ["signed_in", "mint"] {
        assert_eq!(
            json_at(
                &at,
                &format!("conformance/media.serve@1/{issued}-answers.json")
            )
            .pointer("/response/json"),
            Some(&json!("<redacted>")),
            "{issued}"
        );
    }
}
