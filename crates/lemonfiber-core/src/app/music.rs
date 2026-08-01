//! Choosing the audio format for music, and carrying it out.
//!
//! Music has no resolution, so its quality is an [`audio::Format`](crate::audio)
//! rather than a resolution preset — and no community profile configures it, so the
//! choice is applied straight to the music service (Lidarr) through its API. Unlike a
//! resolution preset, which is recorded and picked up when the stack is next written,
//! this both records the choice and applies it now.
//!
//! The choice is recorded first, so it is remembered even when the service cannot be
//! reached; a rehearsal records nothing and applies nothing.

use super::targets::{project_directory, servarr_targets};
use super::Ctx;
use crate::audio::Format;
use crate::error::Problem;
use crate::model::{Disposition, MusicReport, Triggered};
use crate::ports::service::MusicQuality;

/// The compose id of the music service the format is applied to.
const LIDARR: &str = "lidarr";

/// Record the audio format for music and apply it to the music service, reporting what
/// became of both.
///
/// A rehearsal records nothing and applies nothing. Otherwise the choice is saved and
/// then applied best-effort: a service not yet started, or a stack that cannot be read,
/// is reported as not-started rather than failing the command, because the choice is
/// recorded regardless and applying it again once the service is up will reach it.
pub(super) async fn music(ctx: &Ctx, format: Format) -> Result<MusicReport, Box<Problem>> {
    let mut selection = super::quality::load_selection(ctx)?;
    selection.set_music(format);
    let choice = super::quality::music_choice(format);

    if ctx.dry_run {
        return Ok(MusicReport {
            choice,
            disposition: Disposition::Rehearsed,
            outcome: None,
        });
    }

    super::quality::save_selection(ctx, &selection)?;
    let outcome = apply(ctx, format).await;
    Ok(MusicReport {
        choice,
        disposition: Disposition::Recorded,
        outcome: Some(outcome),
    })
}

/// Apply the format to the music service, reading what it said. A stack that cannot be
/// read, or no music service in it, or one that has not finished starting, is reported
/// not-started — the choice is already recorded, and a later run reaches the service.
async fn apply(ctx: &Ctx, format: Format) -> Triggered {
    // A stack that cannot be read yields no targets, so an unreadable stack and one
    // that simply names no music service both arrive here as "no target" — the same
    // not-started truth, since either way there is nothing to apply to.
    let target = ctx
        .stack
        .checked_manifest(ctx.today())
        .ok()
        .map(|manifest| {
            let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
            servarr_targets(&manifest.services, project.as_deref())
        })
        .unwrap_or_default()
        .into_iter()
        .find(|target| target.id == LIDARR);
    let Some(target) = target else {
        return Triggered::NotStarted;
    };
    match target.open(&ctx.http, ctx.filesystem.as_ref()).await {
        None => Triggered::NotStarted,
        Some(service) => match service.apply_music_format(format).await {
            Ok(()) => Triggered::Started,
            Err(failure) => Triggered::Failed {
                detail: failure.to_string(),
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::music;
    use super::Ctx;
    use crate::audio::Format;
    use crate::config::Settings;
    use crate::model::{Disposition, Triggered};
    use crate::platform::Environment;
    use crate::stack::Source;
    use crate::test_support::{spoke, stack, Reporting, Scripted, ScriptedHttp, SeedFs};

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
        Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(Reporting::absent()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            stack(),
            Settings::default(),
            Environment::MacOs,
        )
        .with_filesystem(fs)
        .with_http(Arc::new(ScriptedHttp::new(replies)))
    }

    /// A context with a writable choice file, so recording succeeds.
    fn recording_ctx(fs: Arc<SeedFs>, replies: Vec<(u16, &'static str)>) -> Ctx {
        let dir = std::env::temp_dir().join(format!("lemonfiber-music-{}", std::process::id()));
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
            &recording_ctx(Arc::new(SeedFs::keyed(None, None)), vec![]),
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
            ),
            Format::HiRes,
        )
        .await
        .unwrap_or_default();
        assert!(matches!(report.outcome, Some(Triggered::Failed { .. })));
    }

    #[tokio::test]
    async fn an_unreadable_stack_records_the_choice_and_reports_not_started() {
        let dir = std::env::temp_dir().join(format!("lemonfiber-music-bad-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let mut context = Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(Reporting::absent()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            Source::External(std::path::Path::new("/lemonfiber/no/such/stack")),
            Settings::default(),
            Environment::MacOs,
        )
        .with_filesystem(Arc::new(SeedFs::keyed(None, None)));
        context.settings.env_file = Some(dir.join(".env"));
        let report = music(&context, Format::Lossless).await.unwrap_or_default();
        assert_eq!(report.disposition, Disposition::Recorded);
        assert!(matches!(report.outcome, Some(Triggered::NotStarted)));
    }
}
