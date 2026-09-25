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
pub(crate) async fn music(ctx: &Ctx, format: Format) -> Result<MusicReport, Box<Problem>> {
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
    match target
        .open(&ctx.seams.http, ctx.seams.filesystem.as_ref())
        .await
    {
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
mod tests;
