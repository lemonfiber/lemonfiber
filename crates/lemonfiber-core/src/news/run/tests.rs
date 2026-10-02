use std::sync::Arc;

use crate::app::{conditions, dispatch, Command};
use crate::condition::{Conditions, Fault};
use crate::error::Severity;
use crate::test_support::{a_context, nowhere, spoke, Reporting, Scripted};

use super::news;

/// A context over a stack that will not read, so the household cannot be asked.
fn unread() -> crate::app::Ctx {
    a_context()
        .over(nowhere())
        .runner(Arc::new(Scripted(Ok(spoke("")))))
        .engine(Arc::new(Reporting::default()))
        .build()
}

#[tokio::test]
async fn what_is_new_answers_where_the_household_cannot_be_read() {
    // The kind that could not be read is named, and the rest is said: the releases
    // are compiled in, and what is wrong is kept on this machine.
    let json = dispatch(Command::News, &unread())
        .await
        .ok()
        .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
        .unwrap_or_default();

    assert!(json.contains("\"kind\":\"news-items\""), "{json}");
    assert!(json.contains("\"unread\":[\"requests\"]"), "{json}");
}

#[tokio::test]
async fn a_problem_is_the_one_the_store_of_conditions_holds_with_its_onset() {
    let ctx = crate::app::fixtures::ctx_at("news-problems");
    let mut known = Conditions::new();
    known.observe(
        "service.sonarr",
        Some(&Fault::new(
            "service.stopped",
            Severity::Error,
            "Sonarr is stopped",
            "nothing new arrives for series",
            "start it",
        )),
        "1000",
    );
    conditions::save(&ctx, &known);

    let problems: Vec<(String, String)> = news(&ctx)
        .await
        .problems
        .into_iter()
        .map(|problem| (problem.check, problem.onset))
        .collect();

    assert_eq!(
        problems,
        vec![("service.sonarr".to_owned(), "1000".to_owned())]
    );
}
