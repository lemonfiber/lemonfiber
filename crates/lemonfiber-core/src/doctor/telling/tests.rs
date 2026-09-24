use super::{TellingCheck, BEHIND};
use crate::baseline::Baseline;
use crate::doctor::{Category, Check, Verdict};
use crate::seed::{said, wanted_telling, TELLING};
use crate::seerr::Seerr;
use lemonfiber_fixtures::http::{Answer, Fake};
use std::sync::Arc;

/// A telling that is on, but not for the occasions lemonfiber would choose.
const SOME: &str = r#"{"enabled":true,"types":8}"#;

/// The real client over a scripted service, so the check reads the request this
/// product actually sends.
fn asking(answer: Answer) -> Arc<dyn crate::ports::service::Requests> {
    let http = Fake::by_path_in_turn(vec![("/settings/notifications/webpush", vec![answer])]);
    Arc::new(Seerr::new(http, "http://seerr:5055", "seerr"))
}

fn recorded(value: &str) -> Baseline {
    let mut baseline = Baseline::new();
    baseline.record("seerr", TELLING, value, "2026-08-28T00:00:00Z");
    baseline
}

async fn verdict_for(answer: Answer, baseline: &Baseline) -> Verdict {
    let check = TellingCheck::new(
        Some(asking(answer)),
        baseline.entry("seerr", TELLING).cloned(),
    );
    check
        .run()
        .await
        .into_iter()
        .next()
        .map_or(Verdict::Pass { note: None }, |finding| finding.verdict)
}

#[test]
fn the_household_is_a_question_about_the_services() {
    let check = TellingCheck::new(None, None);
    assert_eq!(check.category(), Category::Services);
}

#[tokio::test]
async fn a_stack_with_no_request_service_has_nothing_to_ask() {
    let check = TellingCheck::new(None, None);
    let found = check.run().await;
    let verdict = found.first().map(|finding| &finding.verdict);

    assert!(
        matches!(verdict, Some(Verdict::Skipped { .. })),
        "{found:?}"
    );
}

#[tokio::test]
async fn a_service_that_will_not_answer_is_unverified_rather_than_a_fault() {
    let verdict = verdict_for(Answer::Silent, &Baseline::new()).await;

    assert!(
        matches!(verdict, Verdict::Unverified { .. }),
        "not knowing was reported as knowing: {verdict:?}"
    );
}

#[tokio::test]
async fn a_telling_nobody_has_set_up_is_seedings_errand_rather_than_a_fault() {
    let verdict = verdict_for(
        Answer::reply(200, r#"{"enabled":false,"types":0}"#),
        &Baseline::new(),
    )
    .await;

    assert!(matches!(verdict, Verdict::Skipped { .. }), "{verdict:?}");
}

#[tokio::test]
async fn a_household_told_everything_passes_and_says_so() {
    let held = format!(
        r#"{{"enabled":true,"types":{}}}"#,
        wanted_telling().occasions
    );
    let verdict = verdict_for(Answer::reply(200, held), &recorded(&said(wanted_telling()))).await;

    assert!(
        matches!(&verdict, Verdict::Pass { note } if note.as_deref().is_some_and(|said| said.contains("decided"))),
        "{verdict:?}"
    );
}

/// Their choice is reported as theirs, both ways round.
///
/// Two cases rather than one, because the sentence differs and the wrong one is
/// worse than none: telling an operator the household hears nothing when they had
/// merely narrowed it would send them looking for a fault that is not there.
#[tokio::test]
async fn a_setting_the_operator_chose_is_reported_as_theirs_whichever_way_they_set_it() {
    // Narrowed: still sending, but not what lemonfiber would choose.
    let narrowed = verdict_for(Answer::reply(200, SOME), &recorded(&said(wanted_telling()))).await;
    assert!(
        matches!(&narrowed, Verdict::Pass { note } if note.as_deref().is_some_and(|said| said.contains("not what"))),
        "{narrowed:?}"
    );

    // Switched off entirely: the household hears nothing, and they chose that.
    let silent = verdict_for(
        Answer::reply(200, r#"{"enabled":false,"types":0}"#),
        &recorded(&said(wanted_telling())),
    )
    .await;
    assert!(
        matches!(&silent, Verdict::Pass { note } if note.as_deref().is_some_and(|said| said.contains("nothing"))),
        "{silent:?}"
    );
}

/// The one outcome worth raising.
///
/// The service holds exactly what lemonfiber last wrote, so nothing looks edited
/// — while what lemonfiber now sends has moved on, and the household hears about
/// less than it should with no sign anything is missing.
#[tokio::test]
async fn lemonfibers_own_value_fallen_behind_is_the_thing_that_warns() {
    let verdict = verdict_for(Answer::reply(200, SOME), &recorded("on:8")).await;

    assert!(
        matches!(&verdict, Verdict::Warn(problem) if problem.code == BEHIND),
        "an operator is not told the household hears less than it should: {verdict:?}"
    );
}
