//! Whether the household will hear back, asked from outside the crate.
//!
//! The check's own tests live beside it and cover the judgement. This drives the
//! same check through the library as the rest of the world links it, because the app
//! layer is compiled twice — once with its in-crate tests, once as the library these
//! binaries link — and an async path exercised from only one of those leaves the
//! other copy counted as never run.
//!
//! Neither home is redundant. Each reaches a mapping the other cannot.

use std::sync::Arc;

use lemonfiber_core::doctor::narrowing::Narrowing;
use lemonfiber_core::doctor::telling::TellingCheck;
use lemonfiber_core::doctor::{examine, Category, Check, Verdict};
use lemonfiber_core::ports::service::Requests;
use lemonfiber_core::seerr::{Seerr, OCCASIONS};
use lemonfiber_fixtures::http::{Answer, Fake};

/// The real client over a scripted request service.
fn asking(answer: Answer) -> Arc<dyn Requests> {
    let http = Fake::by_path_in_turn(vec![("/settings/notifications/webpush", vec![answer])]);
    Arc::new(Seerr::new(http, "http://seerr:5055", "seerr"))
}

/// The one verdict the check produces.
async fn verdict(check: TellingCheck) -> Verdict {
    check
        .run()
        .await
        .into_iter()
        .next()
        .map_or(Verdict::Pass { note: None }, |finding| finding.verdict)
}

#[tokio::test]
async fn a_household_told_everything_reads_as_told() {
    let held = format!(r#"{{"enabled":true,"types":{OCCASIONS}}}"#);
    let check = TellingCheck::new(Some(asking(Answer::reply(200, held))), None);

    assert_eq!(check.category(), Category::Services);
    assert!(
        matches!(verdict(check).await, Verdict::Pass { .. }),
        "a service telling the household everything did not read as telling them"
    );
}

#[tokio::test]
async fn a_stack_without_a_request_service_has_nothing_to_ask() {
    let check = TellingCheck::new(None, None);

    assert!(
        matches!(verdict(check).await, Verdict::Skipped { .. }),
        "a stack with nobody to ask reported something about the household anyway"
    );
}

#[tokio::test]
async fn a_request_service_that_will_not_answer_is_unverified() {
    let check = TellingCheck::new(Some(asking(Answer::Silent)), None);

    assert!(
        matches!(verdict(check).await, Verdict::Unverified { .. }),
        "not knowing what the household is told was reported as knowing"
    );
}

/// The one outcome that warns, driven from outside too.
///
/// The service holds exactly what lemonfiber last wrote and what lemonfiber sends
/// has since moved on, so nothing looks edited while the household hears less than
/// it should. Reached from here as well as in-crate because the arm that builds the
/// warning is only run when this state is, and the library these binaries link is a
/// second copy of it.
#[tokio::test]
async fn a_telling_left_behind_what_lemonfiber_now_sends_warns() {
    let recorded = lemonfiber_core::baseline::Record {
        value: "on:8".to_owned(),
        at: "2026-08-28T00:00:00Z".to_owned(),
        origin: lemonfiber_core::baseline::Origin::Written,
    };
    let check = TellingCheck::new(
        Some(asking(Answer::reply(200, r#"{"enabled":true,"types":8}"#))),
        Some(recorded),
    );

    assert!(
        matches!(verdict(check).await, Verdict::Warn(_)),
        "an operator is not told the household hears less than lemonfiber now sends"
    );
}

/// A setting the operator chose is theirs, and said as theirs.
#[tokio::test]
async fn a_setting_the_operator_chose_is_reported_as_theirs() {
    let recorded = lemonfiber_core::baseline::Record {
        value: format!("on:{OCCASIONS}"),
        at: "2026-08-28T00:00:00Z".to_owned(),
        origin: lemonfiber_core::baseline::Origin::Written,
    };
    let check = TellingCheck::new(
        Some(asking(Answer::reply(200, r#"{"enabled":false,"types":0}"#))),
        Some(recorded),
    );

    assert!(
        matches!(&verdict(check).await, Verdict::Pass { note }
            if note.as_deref().is_some_and(|said| said.contains("how you set it"))),
        "their own choice was not reported as theirs"
    );
}

/// The writing half, driven from outside as well.
///
/// The diagnosis above only reads. This is the step that sets the telling up, and
/// the library these binaries link is a second copy of it — so a service nobody has
/// configured is taken through the write from here too, and what it was asked to
/// send is read back off the request rather than taken on trust.
#[tokio::test]
async fn a_service_nobody_configured_is_set_up_to_tell_the_household() {
    let http = Fake::by_path_in_turn(vec![(
        "/settings/notifications/webpush",
        vec![
            Answer::reply(200, r#"{"enabled":false,"types":0}"#),
            Answer::reply(200, ""),
        ],
    )]);
    let seerr = Seerr::new(http.clone(), "http://seerr:5055", "seerr");

    let (wiring, _) = lemonfiber_core::seed::wire_household_telling(&seerr, None).await;

    assert_eq!(
        wiring.state,
        lemonfiber_core::seed::State::Wired,
        "{wiring:?}"
    );
    let sent = http
        .requests()
        .into_iter()
        .find(|asked| asked.method == lemonfiber_core::ports::http::Method::Post)
        .and_then(|asked| asked.body)
        .unwrap_or_default();
    assert!(
        sent.contains("\"enabled\":true") && sent.contains(&OCCASIONS.to_string()),
        "the household was not asked to be told about anything: {sent}"
    );
}

/// The check as a diagnosis actually runs it.
///
/// Every test above calls the check directly. A run reaches it through a boxed
/// trait object instead, and that is a different instantiation — so the path an
/// operator's `doctor` takes is driven here rather than assumed to be the same one.
#[tokio::test]
async fn a_diagnosis_asks_the_household_question_and_gets_an_answer() {
    let held = format!(r#"{{"enabled":true,"types":{OCCASIONS}}}"#);
    let checks: Vec<Box<dyn Check>> = vec![Box::new(TellingCheck::new(
        Some(asking(Answer::reply(200, held))),
        None,
    ))];

    let report = examine(&checks, &Narrowing::Suite).await;

    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.check == "config.household-telling"),
        "a diagnosis ran and never asked whether the household hears back"
    );
}
