use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use serde_json::json;

use super::{bearer, engine, failed_at, json_at, posted, recorded, source, FRONTED};
use crate::ports::http::PROBLEM;

fn serving_alone() -> String {
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

fn unsupported() -> Answer {
    Answer::served(
        500,
        PROBLEM,
        r#"{"type": "unsupported", "detail": "it mints no keys"}"#,
    )
}

fn serving() -> Arc<Fake> {
    minting(unsupported())
}

fn minting(mint: Answer) -> Arc<Fake> {
    let json = |body: &str| Answer::served(200, "application/json", body);
    Fake::by_path_in_turn(vec![
        ("/health", vec![Answer::reply(200, "ok")]),
        (
            "/lemonfiber/adapter/v1/ready",
            vec![Answer::reply(200, "true")],
        ),
        (
            "/System/Info/Public",
            vec![json("{\"ProductName\": \"Jellyfin Server\"}")],
        ),
        ("/mint", vec![mint]),
        ("/lemonfiber/media.serve/v1/", vec![json("null")]),
    ])
}

fn answered_type(at: &std::path::Path, case: &str) -> Option<serde_json::Value> {
    json_at(
        at,
        &format!("conformance/media.serve@1/{case}-answers.json"),
    )
    .pointer("/response/json/type")
    .cloned()
}

#[tokio::test]
async fn a_case_reading_what_was_not_offered_is_recorded_unsupported_and_never_asked() {
    let at = source("fronted-unoffered", &serving_alone());
    let http = serving();
    let read = recorded(&at, &engine(), &http, Some(7)).await;

    assert_eq!(failed_at(&read), Vec::new());
    for case in ["signed_in", "part_way", "answers_to", "revoke"] {
        assert_eq!(
            answered_type(&at, case),
            Some(json!("unsupported")),
            "{case}"
        );
        assert!(
            posted(&http, &format!("/{case}"))
                .iter()
                .all(|asked| bearer(asked).is_none()),
            "{case} was asked under the key"
        );
    }
    assert_eq!(answered_type(&at, "mint"), Some(json!("unsupported")));
    assert_eq!(posted(&http, "/mint").len(), 1);
}

#[tokio::test]
async fn a_case_reading_a_refused_answer_is_recorded_with_that_refusal_and_never_asked() {
    let at = source("fronted-refused", &serving_alone());
    let http = minting(Answer::served(
        502,
        PROBLEM,
        r#"{"type": "unauthorised", "detail": "the upstream rejected the credential"}"#,
    ));
    let read = recorded(&at, &engine(), &http, Some(7)).await;

    assert_eq!(failed_at(&read), Vec::new());
    for case in ["answers_to", "revoke"] {
        let recording = json_at(
            &at,
            &format!("conformance/media.serve@1/{case}-answers.json"),
        );
        assert_eq!(
            recording.pointer("/response/status"),
            Some(&json!(502)),
            "{case}"
        );
        assert_eq!(
            recording.pointer("/response/json/type"),
            Some(&json!("unauthorised")),
            "{case}"
        );
        assert_eq!(
            recording.pointer("/response/json/detail"),
            Some(&json!(
                "media.serve@1/mint-answers was refused: the upstream rejected the credential"
            )),
            "{case}"
        );
        assert!(
            posted(&http, &format!("/{case}")).is_empty(),
            "{case} was asked"
        );
    }
}

#[tokio::test]
async fn a_case_reading_an_answer_no_refusal_explains_fails_the_run() {
    let at = source("fronted-undeclared", &serving_alone());
    let http = minting(Answer::served(500, "text/plain", "it fell over"));
    let read = recorded(&at, &engine(), &http, Some(7)).await;

    assert!(
        !failed_at(&read).is_empty(),
        "the run went on past an answer nothing declares"
    );
    assert!(posted(&http, "/answers_to").is_empty());
}
