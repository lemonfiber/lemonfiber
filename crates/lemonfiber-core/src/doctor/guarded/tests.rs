use lemonfiber_fixtures::http::{Answer, Fake};

use super::GuardedCheck;
use crate::doctor::{Check as _, Verdict};
use crate::nzbhydra2::Nzbhydra2;

/// The one verdict the check comes to over an aggregator answering `answer`.
async fn verdict(answer: Option<Answer>) -> Option<Verdict> {
    let aggregator = answer.map(|answer| {
        (
            std::sync::Arc::new(Nzbhydra2::new(
                Fake::always(answer),
                "http://127.0.0.1:5076",
                "nzbhydra2",
            )) as std::sync::Arc<dyn super::Exposure>,
            "NZBHydra2".to_owned(),
        )
    });
    GuardedCheck::new(aggregator)
        .run()
        .await
        .into_iter()
        .next()
        .map(|finding| finding.verdict)
}

/// An aggregator answering its configuration to anybody is a warning naming it, what it
/// exposes and what turns its authentication on; one refusing is a pass; one that will
/// not say is a warning too, never a pass; and a stack with none skips the check.
#[tokio::test]
async fn an_aggregator_answering_anybody_is_raised() {
    let exposed = verdict(Some(Answer::reply(200, "{}"))).await;
    let guarded = verdict(Some(Answer::reply(401, ""))).await;
    let silent = verdict(Some(Answer::Silent)).await;
    let none = verdict(None).await;

    assert!(matches!(&exposed, Some(Verdict::Warn(problem))
        if problem.code.as_str() == "CONFIG-7"
            && problem.summary.contains("NZBHydra2")
            && problem.meaning.contains("indexer accounts")
            && problem.remedies.iter().any(|remedy| remedy.action.contains("lemonfiber reset"))));
    assert!(matches!(guarded, Some(Verdict::Pass { .. })));
    assert!(
        matches!(&silent, Some(Verdict::Warn(problem)) if problem.code.as_str() == "CONFIG-7"),
        "{silent:?}"
    );
    assert!(matches!(none, Some(Verdict::Skipped { .. })));
}

/// An answer that is neither the configuration nor a refusal is not taken as guarded.
#[tokio::test]
async fn an_answer_that_is_neither_is_raised() {
    let neither = verdict(Some(Answer::reply(500, ""))).await;

    assert!(
        matches!(&neither, Some(Verdict::Warn(problem)) if problem.code.as_str() == "CONFIG-7"),
        "{neither:?}"
    );
}
