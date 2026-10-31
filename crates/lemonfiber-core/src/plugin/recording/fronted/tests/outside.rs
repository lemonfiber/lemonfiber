use std::path::Path;
use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::proving::Proving;
use lemonfiber_fixtures::support::FixedRandom;
use serde_json::json;

use super::{engine, failed_at, json_at, posted, recorded, source, FRONTED};
use crate::plugin::recording::{record, Recorded, Recorder, Retaking, Took};
use crate::ports::http::{Http, PROBLEM};

const CLAIM: &str = "conformance/owner.claim@1/claim-answers.json";

/// Jellyfin's adapter claiming it with a code an account outside the house shows,
/// declaring `outside` where `host` is named.
fn coded(host: Option<&str>) -> String {
    let outside = host.map_or_else(String::new, |host| {
        format!("outside     = {{ \"owner.claim@1/claim-answers\" = \"{host}\" }}\n")
    });
    FRONTED
        .replace(
            "speaks      = [\"subtitles.fetch@1\"]",
            &format!(
                "speaks      = [\"subtitles.fetch@1\", \"owner.claim@1\"]\nupstream    = {{ code = {{ secret = \"claim\" }} }}\n{outside}"
            ),
        )
        .replace(
            "[[proof]]",
            "[[secret]]\nid            = \"claim\"\nof            = \"jellyfin\"\nwhy           = \"Claim it\"\nlapses_s      = 240\nrecorded_with = \"claim-0000\"\n\n[[proof]]",
        )
}

/// The adapter refusing the stand-in code, as the account outside refuses a lapsed one,
/// and saying nobody holds the part.
fn refusing(claim: Answer) -> Arc<Fake> {
    claiming(vec![claim])
}

/// The adapter answering the claim with each of `claims` in turn, then as [`refusing`].
fn claiming(claims: Vec<Answer>) -> Arc<Fake> {
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
        ("/claim", claims),
        (
            "/owner",
            vec![
                Answer::served(401, "text/plain", "Unauthorized"),
                json("\"open\""),
            ],
        ),
    ])
}

fn unavailable() -> Answer {
    Answer::served(
        503,
        PROBLEM,
        r#"{"type": "unavailable", "detail": "the account is not answering"}"#,
    )
}

fn refused() -> Answer {
    Answer::served(
        500,
        PROBLEM,
        r#"{"type": "refused", "detail": "the account refused the code"}"#,
    )
}

async fn checked(at: &Path, http: &Arc<Fake>) -> Recorded {
    let program = Proving::unreachable();
    let shared = || -> Arc<dyn Http> { Arc::clone(http) as Arc<dyn Http> };
    let taking = Retaking {
        check: true,
        adapter: None,
        built: None,
    };
    let read = record(
        at,
        Recorder {
            program: program.as_ref(),
            serving: &engine(),
            transport: &shared,
            random: &FixedRandom(Some(vec![7; 32])),
            taking: &taking,
            confined: &lemonfiber_adapters::Disk,
        },
    )
    .await;
    assert!(read.is_ok(), "{read:?}");
    read.unwrap_or_else(|_| unreachable!("asserted to be read above"))
}

/// Every recording beneath `at` given a note, as its author writes one.
fn noted(at: &Path) {
    for dir in [
        "fixtures",
        "conformance/owner.claim@1",
        "conformance/subtitles.fetch@1",
    ] {
        for entry in std::fs::read_dir(at.join(dir))
            .into_iter()
            .flatten()
            .flatten()
        {
            let text = std::fs::read_to_string(entry.path()).unwrap_or_default();
            let _ = std::fs::write(
                entry.path(),
                text.replacen("\"note\": \"\"", "\"note\": \"Why.\"", 1),
            );
        }
    }
}

fn took(read: &Recorded, fixture: &str) -> Option<Took> {
    read.recordings
        .iter()
        .find(|one| one.fixture.as_deref() == Some(fixture))
        .map(|one| one.took.clone())
}

#[tokio::test(start_paused = true)]
async fn a_part_claimed_with_a_code_records_the_refusal_of_its_stand_in_and_goes_on_unclaimed() {
    let at = source("fronted-coded", &coded(None));
    let http = refusing(refused());
    let read = recorded(&at, &engine(), &http, Some(7)).await;

    assert_eq!(failed_at(&read), Vec::new());
    assert_eq!(posted(&http, "/claim").len(), 1);
    assert_eq!(
        json_at(&at, CLAIM).pointer("/response/json/type"),
        Some(&json!("refused"))
    );
    assert_eq!(
        json_at(&at, "conformance/owner.claim@1/owner-answers.json").pointer("/response/json"),
        Some(&json!("open"))
    );
}

#[tokio::test(start_paused = true)]
async fn a_check_leaves_a_case_reaching_outside_the_stack_as_recorded_and_names_its_host() {
    let at = source("fronted-outside", &coded(Some("plex.tv")));
    let _ = recorded(&at, &engine(), &refusing(refused()), Some(7)).await;
    noted(&at);
    let before = std::fs::read_to_string(at.join(CLAIM)).unwrap_or_default();

    let http = refusing(Answer::served(200, "application/json", "\"ours\""));
    let read = checked(&at, &http).await;

    assert!(posted(&http, "/claim").is_empty());
    assert_eq!(
        took(&read, CLAIM),
        Some(Took::Left {
            host: "plex.tv".to_owned()
        })
    );
    assert_eq!(std::fs::read_to_string(at.join(CLAIM)).ok(), Some(before));
    assert!(read.holds, "{:?}", read.recordings);
}

#[tokio::test(start_paused = true)]
async fn a_check_refuses_a_case_reaching_outside_the_stack_whose_recording_is_absent() {
    let at = source("fronted-outside-absent", &coded(Some("plex.tv")));
    let _ = recorded(&at, &engine(), &refusing(refused()), Some(7)).await;
    let _ = std::fs::remove_file(at.join(CLAIM));

    let http = refusing(refused());
    let read = checked(&at, &http).await;

    assert!(posted(&http, "/claim").is_empty());
    assert_eq!(
        took(&read, CLAIM),
        Some(Took::Unreproduced {
            why: format!("{CLAIM} holds no recorded answer")
        })
    );
    assert!(!read.holds);
}

#[tokio::test(start_paused = true)]
async fn a_check_refuses_a_case_reaching_outside_the_stack_whose_recording_is_of_a_run() {
    let at = source("fronted-outside-run", &coded(Some("plex.tv")));
    let _ = recorded(&at, &engine(), &refusing(refused()), Some(7)).await;
    let _ = std::fs::write(
        at.join(CLAIM),
        r#"{"recorded_from": "jellyfin@sha256:abc", "note": "Why.", "run": {"args": ["--version"]}, "result": {"exit": 0, "stdout_starts_with": "1"}}"#,
    );

    let read = checked(&at, &refusing(refused())).await;

    assert_eq!(
        took(&read, CLAIM),
        Some(Took::Unreproduced {
            why: format!("{CLAIM} holds no recorded answer")
        })
    );
}

#[tokio::test(start_paused = true)]
async fn a_part_claimed_with_a_code_is_asked_again_only_while_it_is_not_answering() {
    let at = source("fronted-coded-waiting", &coded(None));
    let http = claiming(vec![unavailable(), refused()]);
    let read = recorded(&at, &engine(), &http, Some(7)).await;

    assert_eq!(failed_at(&read), Vec::new());
    assert_eq!(posted(&http, "/claim").len(), 2);
    assert_eq!(
        json_at(&at, CLAIM).pointer("/response/json/type"),
        Some(&json!("refused"))
    );
}

#[tokio::test(start_paused = true)]
async fn a_part_claimed_with_a_code_that_never_answers_fails_at_its_health_limit() {
    let at = source("fronted-coded-silent", &coded(None));
    let http = claiming(vec![unavailable()]);
    let read = recorded(&at, &engine(), &http, Some(7)).await;

    assert_eq!(
        failed_at(&read),
        vec![(
            "claim".to_owned(),
            "it did not answer the claim within its health's limit".to_owned()
        )]
    );
    assert!(posted(&http, "/claim").len() > 1);
}

#[tokio::test(start_paused = true)]
async fn a_part_claimed_with_a_code_taken_only_once_its_upstream_restarts_fails_at_its_claim() {
    let at = source("fronted-coded-restarting", &coded(None));
    let http = claiming(vec![Answer::reply(205, "")]);
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
