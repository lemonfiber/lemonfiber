//! What the services are saying.

use super::*;

/// The lines a log stream carried, in the order it handed them over.
///
/// Shaped like [`heard`] beside it: a closed channel stands in for a stream that
/// could not be opened, so there is no branch here for a test to leave unrun.
async fn spoken(ctx: &Ctx, forms: &[String], query: LogQuery) -> Vec<String> {
    let (closed, silent) = tokio::sync::mpsc::channel(1);
    drop(closed);

    let mut lines = super::super::logs(ctx, forms, &[], query)
        .await
        .unwrap_or(silent);

    let mut seen = Vec::new();
    while let Some(line) = lines.recv().await {
        seen.push(line.line);
    }
    seen
}

/// The services a log stream actually carried lines for.
async fn heard(ctx: &Ctx, forms: &[String], services: &[String]) -> Vec<String> {
    let (closed, silent) = tokio::sync::mpsc::channel(1);
    drop(closed);

    let query = LogQuery::recent(10);
    let mut lines = super::super::logs(ctx, forms, services, query)
        .await
        .unwrap_or(silent);

    let mut seen = Vec::new();
    while let Some(line) = lines.recv().await {
        seen.push(line.service);
    }
    seen.sort();
    seen.dedup();
    seen
}

/// One reader per container means a scrollback arrives in bursts. Read back, it
/// should be one account of what happened rather than three.
#[tokio::test]
async fn a_scrollback_reads_as_one_timeline_rather_than_one_burst_per_service() {
    let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy)
        .saying_at("jellyfin", "2026-08-21T19:00:03.000000000Z", "third")
        .saying_at("jellyfin", "2026-08-21T19:00:04.000000000Z", "fourth")
        .saying_at("seerr", "2026-08-21T19:00:01.000000000Z", "first")
        .saying_at("seerr", "2026-08-21T19:00:02.000000000Z", "second");

    let said = spoken(
        &watching(engine),
        &["library".to_owned()],
        LogQuery::recent(20),
    )
    .await;

    assert_eq!(
        said,
        ["first", "second", "third", "fourth"],
        "the containers' own stamps decide, not which reader finished first"
    );
}

/// A live stream has nothing to sort against, so it is handed straight back.
#[tokio::test]
async fn following_hands_the_stream_back_as_it_arrives() {
    let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy)
        .saying_at("jellyfin", "2026-08-21T19:00:09.000000000Z", "later")
        .saying_at("seerr", "2026-08-21T19:00:01.000000000Z", "earlier");

    let said = spoken(
        &watching(engine),
        &["library".to_owned()],
        LogQuery {
            tail: 20,
            follow: true,
        },
    )
    .await;

    assert_eq!(
        said,
        ["later", "earlier"],
        "arrival order is the only order a live stream has"
    );
}

#[tokio::test]
async fn reading_logs_for_a_form_narrows_to_what_that_form_declares() {
    let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy)
        .saying("jellyfin", "started")
        .saying("sonarr", "also started");
    let ctx = watching(engine);

    assert_eq!(
        heard(&ctx, &["library".to_owned()], &[]).await,
        vec!["jellyfin".to_owned()],
        "a form's log view must not carry another form's output"
    );
}

#[tokio::test]
async fn naming_a_service_narrows_further_still() {
    let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy)
        .saying("jellyfin", "started")
        .saying("seerr", "also started");
    let ctx = watching(engine);

    assert_eq!(
        heard(&ctx, &["library".to_owned()], &["seerr".to_owned()]).await,
        vec!["seerr".to_owned()]
    );
}

#[tokio::test]
async fn naming_no_form_reads_everything_that_is_saying_anything() {
    let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy)
        .saying("jellyfin", "started")
        .saying("sonarr", "also started");
    let ctx = watching(engine);

    assert_eq!(
        heard(&ctx, &[], &[]).await,
        vec!["jellyfin".to_owned(), "sonarr".to_owned()]
    );
}

#[tokio::test]
async fn reading_logs_reports_an_engine_it_cannot_see() {
    let ctx = watching(Reporting::absent());
    let query = LogQuery::recent(10);
    let refusal = super::super::logs(&ctx, &[], &[], query)
        .await
        .err()
        .map(|problem| problem.code);
    assert_eq!(
        refusal,
        Some(crate::error::codes::docker::ENGINE_UNREACHABLE)
    );
}

#[tokio::test]
async fn reading_logs_for_a_form_this_stack_does_not_have_is_refused() {
    let ctx = watching(Reporting::default());
    let query = LogQuery::recent(10);
    let refusal = super::super::logs(&ctx, &["telly".to_owned()], &[], query)
        .await
        .err()
        .map(|problem| problem.code);
    assert_eq!(refusal, Some(crate::error::codes::form::NO_SUCH_FORM));
}

#[tokio::test]
async fn reading_logs_from_a_stack_that_cannot_be_read_is_refused() {
    let nowhere = Source::External(std::path::Path::new("/lemonfiber/no/such/stack"));
    let ctx = a_context()
        .engine(Arc::new(Reporting::default()))
        .over(nowhere)
        .build();
    let query = LogQuery::recent(10);
    assert_eq!(
        super::super::logs(&ctx, &[], &[], query)
            .await
            .err()
            .map(|problem| problem.code),
        Some(crate::error::codes::stack::STACK_UNREADABLE)
    );
}
