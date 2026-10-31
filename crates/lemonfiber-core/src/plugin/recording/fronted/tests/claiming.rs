use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use serde_json::json;

use super::{engine, failed_at, json_at, posted, recorded, source, FRONTED};
use crate::ports::http::PROBLEM;

fn claimed_first() -> String {
    FRONTED.replace(
        "speaks      = [\"subtitles.fetch@1\"]",
        "speaks      = [\"subtitles.fetch@1\", \"owner.claim@1\"]",
    )
}

fn owned(claim: Vec<Answer>, owner: Vec<Answer>) -> Arc<Fake> {
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
        (
            "/watching",
            vec![
                Answer::served(401, "text/plain", "Unauthorized"),
                json("[]"),
            ],
        ),
        ("/watch", vec![json("null")]),
        ("/claim", claim),
        ("/owner", owner),
    ])
}

#[tokio::test(start_paused = true)]
async fn a_part_claimed_first_is_claimed_before_its_cases_and_they_are_what_the_claim_heard() {
    let at = source("fronted-claimed", &claimed_first());
    let json = |body: &str| Answer::served(200, "application/json", body);
    let http = owned(
        vec![json("\"settling\"")],
        vec![
            Answer::served(
                503,
                PROBLEM,
                r#"{"type": "unavailable", "detail": "it is restarting"}"#,
            ),
            json("\"ours\""),
            Answer::served(401, "text/plain", "Unauthorized"),
        ],
    );
    let read = recorded(&at, &engine(), &http, Some(7)).await;

    assert_eq!(failed_at(&read), Vec::new());
    assert_eq!(posted(&http, "/claim").len(), 1);
    for (case, answered) in [("claim", json!("settling")), ("owner", json!("ours"))] {
        assert_eq!(
            json_at(
                &at,
                &format!("conformance/owner.claim@1/{case}-answers.json")
            )
            .pointer("/response/json"),
            Some(&answered),
            "{case}"
        );
    }
}

#[tokio::test]
async fn a_part_somebody_else_holds_fails_the_service_at_its_claim() {
    let at = source("fronted-theirs", &claimed_first());
    let http = owned(
        vec![Answer::served(200, "application/json", "\"theirs\"")],
        vec![Answer::served(401, "text/plain", "Unauthorized")],
    );
    let read = recorded(&at, &engine(), &http, Some(7)).await;

    assert_eq!(
        failed_at(&read),
        vec![("claim".to_owned(), "it was Theirs once claimed".to_owned())]
    );
}

#[tokio::test(start_paused = true)]
async fn a_part_still_settling_at_its_health_limit_fails_the_service_at_its_claim() {
    let at = source("fronted-settling", &claimed_first());
    let settling = || vec![Answer::served(200, "application/json", "\"settling\"")];
    let http = owned(settling(), settling());
    let read = recorded(&at, &engine(), &http, Some(7)).await;

    assert_eq!(
        failed_at(&read),
        vec![(
            "claim".to_owned(),
            "it was not claimed within its health's limit".to_owned()
        )]
    );
    assert_eq!(posted(&http, "/claim").len(), 1);
}

#[tokio::test]
async fn a_claim_the_adapter_refuses_fails_the_service_at_it_saying_why() {
    let at = source("fronted-refused", &claimed_first());
    let http = owned(
        vec![Answer::served(401, "text/plain", "Unauthorized")],
        vec![Answer::served(401, "text/plain", "Unauthorized")],
    );
    let read = recorded(&at, &engine(), &http, Some(7)).await;

    assert_eq!(
        failed_at(&read),
        vec![(
            "claim".to_owned(),
            "jellyfin-adapter rejected the credential".to_owned()
        )]
    );
}

#[tokio::test]
async fn a_claim_taken_only_once_the_upstream_restarts_fails_the_service_at_it() {
    let at = source("fronted-restarting", &claimed_first());
    let http = owned(
        vec![Answer::reply(205, "")],
        vec![Answer::served(401, "text/plain", "Unauthorized")],
    );
    let read = recorded(&at, &engine(), &http, Some(7)).await;

    assert_eq!(
        failed_at(&read),
        vec![(
            "claim".to_owned(),
            "it takes the claim only once its upstream starts again, which a recording does not do"
                .to_owned()
        )]
    );
    assert_eq!(posted(&http, "/claim").len(), 1);
}
