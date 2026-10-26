//! Choosing the audio format for music, and carrying it out.
//!
//! Music has no resolution, so its quality is an [`audio::Format`](crate::audio)
//! rather than a resolution preset — and no community profile configures it, so the
//! choice is applied straight to the music service through its API. Unlike a
//! resolution preset, which is recorded and picked up when the stack is next written,
//! this both records the choice and applies it now.
//!
//! The choice is recorded first, so it is remembered even when the service cannot be
//! reached; a rehearsal records nothing and applies nothing.

use super::targets::fillers_here;
use super::Ctx;
use crate::audio::Format;
use crate::error::Problem;
use crate::model::{Disposition, MusicReport, Triggered};
use crate::ports::media::MUSIC;
use crate::ports::service::MusicQuality;

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
            rehearsed: false,
            choice,
            disposition: Disposition::Rehearsed,
            outcome: None,
            changes: crate::quality::CHANGES.to_owned(),
        });
    }

    super::quality::save_selection(ctx, &selection)?;
    let outcome = apply(ctx, format).await;
    Ok(MusicReport {
        rehearsed: false,
        choice,
        disposition: Disposition::Recorded,
        outcome: Some(outcome),
        changes: crate::quality::CHANGES.to_owned(),
    })
}

/// Apply the format to the music service, reading what it said. A stack that cannot be
/// read, or no music service in it, or one that has not finished starting, is reported
/// not-started — the choice is already recorded, and a later run reaches the service.
async fn apply(ctx: &Ctx, format: Format) -> Triggered {
    let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) else {
        return Triggered::NotStarted;
    };
    let fillers = fillers_here(ctx, &manifest);
    let Some(curator) = crate::seed::run::curators(&fillers)
        .into_iter()
        .find(|curator| curator.media_types().iter().any(|media| media == MUSIC))
    else {
        return Triggered::NotStarted;
    };
    match curator.client(ctx).await {
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
