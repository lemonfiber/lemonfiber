//! Asking the indexers, and what the accounts say about a stall.

use super::*;

/// One monitored item, with nothing yet done about it.
const MONITORED: &str = r#"[{"id":1,"title":"The Expanse","monitored":true}]"#;

/// A wanted-missing page naming one item, which is what a release search is run for.
const ONE_WANTED: &str = r#"{"records":[{"id":7}]}"#;

/// A wanted-missing page with nothing on it, so there is nothing to search for.
const NOTHING_WANTED: &str = r#"{"records":[]}"#;

/// A release search that came back with releases the profile rejected every one of.
const ALL_REJECTED: &str = r#"[{"rejections":["quality 1080p is not wanted"]}]"#;

/// A release search that came back with a release the profile would grab.
const ONE_ACCEPTED: &str = r#"[{"rejections":[]}]"#;

/// A release search that ran cleanly and came back with nothing at all.
const NO_RELEASES: &str = "[]";

/// A context whose download client's key is readable as well as the \*arrs', so a
/// trace that asks the accounts resolves both readers rather than only one.
fn ctx_with_accounts(fake: &Fake) -> Ctx {
    ctx_with(fake).with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), Some(SAB_INI))))
}

/// The distinction this whole path exists to draw: releases are out there and the
/// quality in force rejects every one of them.
///
/// That is a different situation from an indexer carrying nothing, and it has a
/// different remedy — the operator can ease the preset — so the item reads as having
/// reached `Found`, the stage the pipeline has for exactly this, and the reason names
/// the quality rather than the indexers.
#[tokio::test]
async fn releases_the_quality_rejects_read_as_found_rather_than_as_nothing_found() {
    let report = searched_for(&Fake::searching(MONITORED, ONE_WANTED, ALL_REJECTED)).await;

    assert_eq!(report.furthest, Stage::Found);
    assert_eq!(
        report.stages.last().map(|stage| stage.stage),
        Some(Stage::Found)
    );
    let reason = report.stall.unwrap_or_default();
    assert!(reason.contains("quality preset"), "{reason}");
    assert!(reason.contains("easing"), "{reason}");
    // And never the generic one it would have carried unasked, which claims the
    // question is open when the search has just closed it.
    assert!(!reason.contains("no search was run"), "{reason}");
}

/// The other half of the same distinction, and the reason it is worth drawing: a
/// clean search that found nothing at all says so, and says it is not the preset's
/// doing — so an operator does not go and ease a quality that rejected nothing.
#[tokio::test]
async fn a_search_that_found_nothing_says_the_indexers_carry_nothing() {
    let report = searched_for(&Fake::searching(MONITORED, ONE_WANTED, NO_RELEASES)).await;

    assert_eq!(report.furthest, Stage::Monitored);
    let reason = report.stall.unwrap_or_default();
    assert!(reason.contains("a search found nothing"), "{reason}");
    assert!(reason.contains("not the quality preset"), "{reason}");
}

/// A search that settled nothing is neither of the two, and says so.
///
/// Nothing was wanted to search for, so what came back is about no item at all — and
/// reading that as an empty indexer would send an operator after a fault that has not
/// been shown to exist.
#[tokio::test]
async fn a_search_that_settled_nothing_leaves_the_question_open_and_says_it_asked() {
    let report = searched_for(&Fake::searching(MONITORED, NOTHING_WANTED, NO_RELEASES)).await;

    assert_eq!(report.furthest, Stage::Monitored);
    let reason = report.stall.unwrap_or_default();
    assert!(reason.contains("settled nothing"), "{reason}");
}

/// A search whose answer is about content other than the item followed settles
/// nothing about that item either, however healthy it reads.
#[tokio::test]
async fn a_search_that_answers_about_other_content_settles_nothing_here() {
    let report = searched_for(&Fake::searching(MONITORED, ONE_WANTED, ONE_ACCEPTED)).await;

    assert_eq!(report.furthest, Stage::Monitored);
    assert!(report
        .stall
        .as_deref()
        .is_some_and(|reason| reason.contains("settled nothing")));
}

/// A search that will not run establishes nothing, and is never read as an absence.
///
/// An indexer that could not be reached is not an indexer carrying nothing, and the
/// trace that confused the two would send its operator to ease a preset that was
/// never the problem.
#[tokio::test]
async fn a_search_that_will_not_run_is_not_an_indexer_carrying_nothing() {
    let report = searched_for(&Fake::searching(MONITORED, "not json", NO_RELEASES)).await;

    assert_eq!(report.furthest, Stage::Monitored);
    assert!(report
        .stall
        .as_deref()
        .is_some_and(|reason| reason.contains("settled nothing")));
}

/// The gate itself: nothing is asked of the indexers unless it was asked for.
///
/// Asserted on the request having been made rather than on the reading it would have
/// produced, because the cost is the request — a search spent against the allowance
/// the indexers hold the operator to is spent whatever the trace then says.
#[tokio::test]
async fn a_trace_nobody_asked_to_search_asks_the_indexers_nothing() {
    let fake = Fake::searching(MONITORED, ONE_WANTED, ALL_REJECTED);
    let transport = fake.transport();
    let context = ctx_with(&fake).with_http(Arc::<Transport>::clone(&transport));

    let report = trace(&context, "expanse", None, false)
        .await
        .unwrap_or_default();

    assert!(
        !transport.asked_for("/release"),
        "the indexers were searched"
    );
    assert_eq!(report.furthest, Stage::Monitored);
}

/// And nothing is asked of them for an item that already has its answer.
///
/// A search says nothing about something already imported, so spending one on it
/// would cost the operator an indexer request for a question that is not open.
#[tokio::test]
async fn a_trace_that_already_has_its_answer_asks_the_indexers_nothing() {
    let fake = Fake {
        history: r#"{"records":[{"eventType":"downloadFolderImported","date":"2026-01-01T00:00:00Z"}]}"#,
        ..Fake::searching(MONITORED, ONE_WANTED, ALL_REJECTED)
    };
    let transport = fake.transport();
    let context = ctx_with(&fake).with_http(Arc::<Transport>::clone(&transport));

    let report = trace(&context, "expanse", None, true)
        .await
        .unwrap_or_default();

    assert!(
        !transport.asked_for("/release"),
        "the indexers were searched"
    );
    assert_eq!(report.furthest, Stage::Imported);
}

/// A stall the accounts could explain, on a stack whose accounts will not answer: the
/// reason stands exactly as it did. A service that could not be read says nothing about
/// the accounts behind it, and a trace that added an empty aside would be claiming it
/// had asked and heard something.
#[tokio::test]
async fn a_stall_reads_unchanged_where_the_accounts_cannot_be_read() {
    let context = ctx_with_accounts(&Fake::arr(
        r#"[{"id":1,"title":"The Expanse","monitored":true}]"#,
        r#"{"records":[]}"#,
        EMPTY_QUEUE,
    ));
    let report = trace(&context, "expanse", None, false)
        .await
        .unwrap_or_default();
    assert_eq!(report.furthest, Stage::Monitored);
    assert!(report
        .stall
        .as_deref()
        .is_some_and(|reason| reason.contains("no search was run") && !reason.contains('(')));
}

/// The two stages an account can explain, and the ones it cannot. A preset that finds
/// nothing good enough is not an account problem, and sending its operator to look at
/// their subscriptions would be the wrong half of the answer.
#[test]
fn only_the_stalls_an_account_could_explain_ask_the_accounts() {
    let stalled = |furthest: Stage| TraceReport {
        furthest,
        stall: Some("stopped".to_owned()),
        ..TraceReport::default()
    };
    assert_eq!(
        account_explainable(&stalled(Stage::Monitored)).as_deref(),
        Some("stopped")
    );
    assert_eq!(
        account_explainable(&stalled(Stage::Grabbed)).as_deref(),
        Some("stopped")
    );
    assert_eq!(account_explainable(&stalled(Stage::Found)), None);
    assert_eq!(account_explainable(&stalled(Stage::Imported)), None);
    // Progressing rather than stopped: there is nothing to explain.
    assert_eq!(
        account_explainable(&TraceReport {
            furthest: Stage::Monitored,
            stall: None,
            ..TraceReport::default()
        }),
        None
    );
}

/// An account that is fine, or one nothing could be read from, explains nothing about
/// why an item stopped — and saying so beside a stall would bury the reason.
#[test]
fn only_the_findings_that_want_acting_on_are_carried() {
    let said = troubles(vec![
        finding(
            "Fast Indexer",
            Verdict::Warn(problem("An indexer has used everything it allows for now")),
        ),
        finding(
            "Block 500",
            Verdict::Fail(problem("A Usenet account is refusing the login")),
        ),
        finding("Quiet", Verdict::Pass { note: None }),
        finding(
            "Unread",
            Verdict::Unverified {
                reason: "nothing answered".to_owned(),
                remedy: Remedy::new("try again"),
            },
        ),
    ]);
    assert_eq!(
        said,
        vec![
            "Fast Indexer — An indexer has used everything it allows for now".to_owned(),
            "Block 500 — A Usenet account is refusing the login".to_owned(),
        ]
    );
}

/// Beside rather than instead: how far the item got is still the answer to the
/// question that was asked.
#[test]
fn what_the_accounts_say_travels_beside_the_stall() {
    let reason = "monitored, but nothing has been grabbed for it yet".to_owned();
    assert_eq!(beside(reason.clone(), &[]), reason);
    assert_eq!(
        beside(
            reason,
            &["Fast — capped".to_owned(), "Slow — refused".to_owned()]
        ),
        "monitored, but nothing has been grabbed for it yet (Fast — capped; Slow — refused)"
    );
}

/// A problem whose summary is what a stall would quote.
fn problem(summary: &str) -> Problem {
    Problem::new(
        crate::error::codes::provider::PROVIDER_EMPTY,
        Severity::Warning,
        summary,
        "why it matters",
        Remedy::new("do something"),
    )
}
