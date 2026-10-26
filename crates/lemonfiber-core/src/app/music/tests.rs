use super::music;
use super::Ctx;
use crate::audio::Format;
use crate::model::{Disposition, Triggered};
use crate::test_support::curators::{curating, refusing, taken, MUSIC};
use crate::test_support::{a_context, nowhere};
use lemonfiber_fixtures::http::{Answer, Fake};

/// A context over the real stack, which names a music curator, with HTTP answering
/// nothing.
fn ctx() -> Ctx {
    a_context().build().with_http(Fake::silent())
}

/// A context with a writable choice file, so recording succeeds, and the music curator
/// moved into its plugin, its adapter answering the format's application as `applied`.
/// `tag` names a directory of this test's own: the choice file is a real file on disk, so
/// tests sharing one directory would race on it when run in parallel.
fn recording_ctx(applied: Answer, tag: &str) -> Ctx {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("music-{tag}")).kept();
    let _ = std::fs::create_dir_all(&dir);
    let mut context = a_context().build().with_http(Fake::by_path(vec![(
        curating(MUSIC, "apply_music_format"),
        applied,
    )]));
    context.settings.env_file = Some(dir.join(".env"));
    MUSIC.reaching(context)
}

#[tokio::test]
async fn a_rehearsal_records_nothing_and_applies_nothing() {
    let mut context = ctx();
    context.dry_run = true;
    let report = music(&context, Format::Lossless).await.unwrap_or_default();
    assert_eq!(report.disposition, Disposition::Rehearsed);
    assert!(report.outcome.is_none());
    assert_eq!(report.choice.format, Format::Lossless.label());
}

#[tokio::test]
async fn a_recorded_choice_is_applied_to_the_music_service() {
    let report = music(&recording_ctx(taken(), "applied"), Format::Lossless)
        .await
        .unwrap_or_default();
    assert_eq!(report.disposition, Disposition::Recorded);
    assert!(matches!(report.outcome, Some(Triggered::Started)));
}

#[tokio::test]
async fn a_service_not_yet_started_is_reported_not_started() {
    let mut context = recording_ctx(taken(), "not-started");
    context.seams.engine = std::sync::Arc::new(lemonfiber_fixtures::support::Reporting::holding(
        &[],
        lemonfiber_ports::docker::Lifecycle::Running,
        lemonfiber_ports::docker::Health::Healthy,
    ));
    let report = music(&context, Format::Compact).await.unwrap_or_default();
    assert_eq!(report.disposition, Disposition::Recorded);
    assert!(matches!(report.outcome, Some(Triggered::NotStarted)));
}

#[tokio::test]
async fn a_service_that_refuses_the_change_is_reported_failed() {
    let report = music(&recording_ctx(refusing(), "refused"), Format::HiRes)
        .await
        .unwrap_or_default();
    assert!(matches!(report.outcome, Some(Triggered::Failed { .. })));
}

#[tokio::test]
async fn an_unreadable_stack_records_the_choice_and_reports_not_started() {
    let dir = lemonfiber_fixtures::scratch::Scratch::named("music-bad");
    let _ = std::fs::create_dir_all(&dir);
    let mut context = a_context().over(nowhere()).build();
    context.settings.env_file = Some(dir.join(".env"));
    let report = music(&context, Format::Lossless).await.unwrap_or_default();
    assert_eq!(report.disposition, Disposition::Recorded);
    assert!(matches!(report.outcome, Some(Triggered::NotStarted)));
}

/// A stack whose music curator has not moved into its plugin has no curator to apply the
/// format to, so the choice is recorded and the outcome is not started.
#[tokio::test]
async fn a_stack_with_no_music_curator_reports_not_started() {
    let dir = lemonfiber_fixtures::scratch::Scratch::named("music-none").kept();
    let _ = std::fs::create_dir_all(&dir);
    let mut context = ctx();
    context.settings.env_file = Some(dir.join(".env"));
    let report = music(&context, Format::Lossless).await.unwrap_or_default();
    assert_eq!(report.disposition, Disposition::Recorded);
    assert!(matches!(report.outcome, Some(Triggered::NotStarted)));
}
