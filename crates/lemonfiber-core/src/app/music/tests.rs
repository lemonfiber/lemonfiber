use std::sync::Arc;

use super::music;
use super::Ctx;
use crate::audio::Format;
use crate::model::{Disposition, Triggered};
use crate::test_support::{a_context, nowhere, SeedFs};
use lemonfiber_fixtures::http::Fake;

/// The Lidarr config file `SeedFs` hands back, carrying a readable key so the target
/// opens.
const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";

/// A Lidarr default-shaped quality profile the apply fetches and rewrites.
const PROFILE: &str = r#"[{"id":1,"upgradeAllowed":false,"cutoff":1006,"items":[
    {"id":1006,"name":"Lossless","allowed":false,"items":[
        {"quality":{"id":6,"name":"FLAC"},"items":[],"allowed":false}
    ]}
]}]"#;

/// A context over the real stack (which names Lidarr), the given filesystem, and HTTP
/// answering from `replies`.
fn ctx(fs: Arc<SeedFs>, replies: Vec<(u16, &'static str)>) -> Ctx {
    a_context()
        .build()
        .with_filesystem(fs)
        .with_http(Fake::scripted(replies))
}

/// A context with a writable choice file, so recording succeeds. `tag` names a
/// directory of this test's own: the choice file is a real file on disk, so tests
/// sharing one directory would race on it when run in parallel — the tag keeps each
/// test's `.env` to itself.
fn recording_ctx(fs: Arc<SeedFs>, replies: Vec<(u16, &'static str)>, tag: &str) -> Ctx {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("music-{tag}")).kept();
    let _ = std::fs::create_dir_all(&dir);
    let mut context = ctx(fs, replies);
    context.settings.env_file = Some(dir.join(".env"));
    context
}

#[tokio::test]
async fn a_rehearsal_records_nothing_and_applies_nothing() {
    let mut context = ctx(Arc::new(SeedFs::keyed(None, None)), vec![]);
    context.dry_run = true;
    let report = music(&context, Format::Lossless).await.unwrap_or_default();
    assert_eq!(report.disposition, Disposition::Rehearsed);
    assert!(report.outcome.is_none());
    assert_eq!(report.choice.format, Format::Lossless.label());
}

#[tokio::test]
async fn a_recorded_choice_is_applied_to_the_music_service() {
    // The profile GET is answered, then the PUT; Lidarr accepts both.
    let report = music(
        &recording_ctx(
            Arc::new(SeedFs::keyed(Some(KEYED), None)),
            vec![(200, PROFILE), (200, "")],
            "applied",
        ),
        Format::Lossless,
    )
    .await
    .unwrap_or_default();
    assert_eq!(report.disposition, Disposition::Recorded);
    assert!(matches!(report.outcome, Some(Triggered::Started)));
}

#[tokio::test]
async fn a_service_not_yet_started_is_reported_not_started() {
    // No key on disk: the target does not open, so nothing is applied — but the
    // choice is still recorded.
    let report = music(
        &recording_ctx(Arc::new(SeedFs::keyed(None, None)), vec![], "not-started"),
        Format::Compact,
    )
    .await
    .unwrap_or_default();
    assert_eq!(report.disposition, Disposition::Recorded);
    assert!(matches!(report.outcome, Some(Triggered::NotStarted)));
}

#[tokio::test]
async fn a_service_that_refuses_the_change_is_reported_failed() {
    let report = music(
        &recording_ctx(
            Arc::new(SeedFs::keyed(Some(KEYED), None)),
            vec![(200, PROFILE), (500, "boom")],
            "refused",
        ),
        Format::HiRes,
    )
    .await
    .unwrap_or_default();
    assert!(matches!(report.outcome, Some(Triggered::Failed { .. })));
}

#[tokio::test]
async fn an_unreadable_stack_records_the_choice_and_reports_not_started() {
    let dir = lemonfiber_fixtures::scratch::Scratch::named("music-bad");
    let _ = std::fs::create_dir_all(&dir);
    let mut context = a_context()
        .over(nowhere())
        .build()
        .with_filesystem(Arc::new(SeedFs::keyed(None, None)));
    context.settings.env_file = Some(dir.join(".env"));
    let report = music(&context, Format::Lossless).await.unwrap_or_default();
    assert_eq!(report.disposition, Disposition::Recorded);
    assert!(matches!(report.outcome, Some(Triggered::NotStarted)));
}
