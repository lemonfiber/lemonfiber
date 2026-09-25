//! Choosing a quality preset, and saying what it costs before it takes hold.
//!
//! The operator's question is small — how good should this look, how much disk am
//! I willing to spend — and this is where they answer it, in the plain language of
//! [`crate::quality`] rather than the tool's. Showing states what each preset means
//! and what it costs; setting records the choice, first stating the consequence for
//! a household this host cannot smoothly serve, and holding a choice that would be
//! CPU-transcoded until the operator confirms it deliberately.
//!
//! The choice is recorded, not applied here: it decides what future acquisitions
//! aim for, and the stack picks it up when it is next written. Nothing on disk is
//! rewritten to match it, and nothing already downloaded is touched.
//!
//! Whether the choice already on record would be transcoded here is answered from
//! this module as well, for the playback guidance that carries the same caution long
//! after a confirmation is forgotten — one judgement read by two surfaces rather than
//! two that can drift.

use std::path::PathBuf;

use crate::app::{Ctx, QualityAction};
use crate::audio::Format;
use crate::config::{store, JELLYFIN_MODE_KEY};
use crate::error::{Diagnose, Problem};
use crate::model::{Disposition, MusicChoice, PresetChoice, QualityReport};
use crate::quality::{Preset, Selection};
use crate::transcoding::{warn_before_confirming, Playback, Warning};
use crate::wizard::Library;

/// The scope a global choice is reported under, as opposed to a media type's name.
const EVERYTHING: &str = "everything";

/// The scope the music format is reported under — its own axis, not a resolution
/// media type.
const MUSIC: &str = "music";

/// Show or change the quality preset.
///
/// A rehearsal reports the choice it would record without writing it, which is what
/// `--dry-run` means everywhere. The confirmation is not the same shape: a choice is
/// recorded whether or not one is given, and what the agreement answers is the one
/// cost the choice can carry — a preset this host would have to transcode in
/// software, which is held rather than recorded until it is agreed to, and which is
/// no cost at all on a host that transcodes in hardware. So an unconfirmed run here
/// is the write, not an account of one. A reapply re-asserts the recorded preset
/// over a Recyclarr config the operator hand-edited, the explicit consent an
/// ordinary run withholds.
pub(crate) fn quality(ctx: &Ctx, action: QualityAction) -> Result<QualityReport, Box<Problem>> {
    let mut selection = load_selection(ctx)?;
    let playback = playback(ctx);
    let record = materialised_record(ctx);
    let into = ctx.settings.stack_dir.as_deref();

    let (disposition, customised, overwritten) = match action {
        QualityAction::Show => (
            Disposition::Shown,
            crate::app::materialise::recyclarr_customised(into, record.as_deref()),
            None,
        ),
        QualityAction::Set {
            preset,
            media_type,
            confirm,
        } => {
            match media_type.as_deref() {
                Some(media_type) => selection.set_type(media_type, preset),
                None => selection.set_global(preset),
            }
            // Held sits above the rehearsal check on purpose: a choice this host
            // would software-transcode would not proceed even for real without
            // confirmation, so a rehearsal of it reports that truth — "this needs
            // confirming" — rather than the "would save" it cannot honestly claim.
            let disposition = if warn_before_confirming(preset, playback).is_some() && !confirm {
                Disposition::Held
            } else if ctx.dry_run {
                Disposition::Rehearsed
            } else {
                save_selection(ctx, &selection)?;
                Disposition::Recorded
            };
            (
                disposition,
                crate::app::materialise::recyclarr_customised(into, record.as_deref()),
                None,
            )
        }
        QualityAction::Reapply => {
            let overwritten = crate::app::materialise::reapply_recyclarr(
                ctx.stack,
                into,
                record.as_deref(),
                &selection,
                &ctx.settings.unmanaged,
                ctx.dry_run,
            )
            .map_err(|failure| Box::new(failure.problem()))?;
            let disposition = if ctx.dry_run {
                Disposition::WouldReapply
            } else {
                Disposition::Reapplied
            };
            // One answer rather than two: a reapply overwrote an edit exactly when it
            // has one to show, so the word and the diff cannot come apart.
            (disposition, overwritten.is_some(), overwritten)
        }
    };

    Ok(report(
        &selection,
        playback,
        customised,
        disposition,
        overwritten,
    ))
}

/// Where the record of what lemonfiber last wrote to the stack is kept — beside the
/// environment file, the same derivation the lifecycle path uses.
fn materialised_record(ctx: &Ctx) -> Option<PathBuf> {
    crate::app::targets::beside_env(ctx, "materialised.json")
}

/// The choice on record, or the default where none has been made.
///
/// An absent file is a first run and reads as the default. A file that is present
/// but cannot be read or parsed is not: lemonfiber will not guess at a choice it
/// cannot read, because guessing the default here and then recording over it would
/// lose the operator's real choice silently. So it is surfaced — the command
/// refuses to overwrite it, and a lifecycle apply refuses to guess a preset it
/// cannot read rather than silently reverting one it already applied — the same
/// stance the settings store takes.
pub(crate) fn load_selection(ctx: &Ctx) -> Result<Selection, Box<Problem>> {
    let default = || Selection::everywhere(Preset::default_preset());
    let Some(path) = quality_path(ctx) else {
        return Ok(default());
    };
    match std::fs::read_to_string(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(default()),
        Err(error) => Err(Box::new(unreadable(path, error.to_string()))),
        Ok(text) => serde_json::from_str(&text)
            .map_err(|error| Box::new(unreadable(path, error.to_string()))),
    }
}

/// The problem a present-but-unreadable choice file raises — reusing the settings
/// store's own "a config file could not be read" failure, since this is one.
fn unreadable(path: PathBuf, reason: String) -> Problem {
    store::Failure::Unreadable { path, reason }.problem()
}

/// The recorded choice, or the default where none is chosen or it cannot be read.
///
/// Best-effort by design: a read (a storage projection, an upgrade's cost) must not
/// fail over an unreadable choice the way a `set` does — it writes nothing, so
/// falling back to the default loses nothing.
pub(crate) fn recorded_selection(ctx: &Ctx) -> Selection {
    load_selection(ctx).unwrap_or_else(|_| Selection::everywhere(Preset::default_preset()))
}

/// The most demanding preset in force — the basis a storage projection turns on.
pub(crate) fn most_demanding_or_default(ctx: &Ctx) -> Preset {
    recorded_selection(ctx).most_demanding()
}

/// Whether what is already chosen asks this host for transcoding it can only do on
/// the processor — the same question a `set` answers before recording a choice,
/// asked here of the choice already on record.
///
/// The most demanding preset in force rather than the global one: a household that
/// left television at balanced and put film at maximum still meets this on film
/// night, and a caution that missed it would be wrong on exactly the evening it is
/// wanted.
///
/// Everything it reads is best-effort. A choice file that cannot be read falls back
/// to the default, which asks for no transcoding, and an unreadable environment file
/// reads as no media server — so a machine with nothing set up warrants no caution
/// rather than refusing to answer.
pub(crate) fn straining(ctx: &Ctx) -> Option<Warning> {
    warn_before_confirming(most_demanding_or_default(ctx), playback(ctx))
}

/// Record the choice where the next run — and a backup — will find it.
///
/// Unlike the baseline seeding keeps best-effort, this is the operator's explicit
/// action, so a place it cannot be written is reported rather than swallowed.
pub(crate) fn save_selection(ctx: &Ctx, selection: &Selection) -> Result<(), Box<Problem>> {
    let path = quality_path(ctx).ok_or_else(|| Box::new(store::Failure::Nowhere.problem()))?;
    store::write(&path, &serde_json::to_string(selection).unwrap_or_default())
        .map_err(|failure| Box::new(failure.problem()))
}

/// Where the choice is kept: beside the environment file, in the configuration
/// directory a backup captures, or nowhere when nothing is configured. Derived from
/// the one path the context carries, and equal to
/// [`crate::config::paths::Paths::quality`].
fn quality_path(ctx: &Ctx) -> Option<PathBuf> {
    crate::app::targets::beside_env(ctx, "quality.json")
}

/// What the media server this stack runs can do with content a client must
/// transcode — read from the platform and the recorded Jellyfin mode.
fn playback(ctx: &Ctx) -> Playback {
    Playback::of(ctx.environment, current_library(ctx))
}

/// The Jellyfin mode on record, or [`Library::None`] where none is — an absent
/// file, an absent key, or a value that is neither mode all mean no server this
/// warning need speak for.
fn current_library(ctx: &Ctx) -> Library {
    ctx.settings
        .env_file
        .as_deref()
        .and_then(|path| store::read(path).ok())
        .and_then(|file| file.get(JELLYFIN_MODE_KEY).and_then(Library::from_mode))
        .unwrap_or(Library::None)
}

/// The choice as a report: the global preset first, then each media type set apart
/// from it, each stating what it means and whether this host can play it smoothly.
fn report(
    selection: &Selection,
    playback: Playback,
    customised: bool,
    disposition: Disposition,
    overwritten: Option<crate::model::StackEdit>,
) -> QualityReport {
    let mut choices = vec![choice(EVERYTHING, selection.global(), playback)];
    for (media_type, preset) in selection.overrides() {
        choices.push(choice(media_type, preset, playback));
    }
    let music = selection
        .music_chosen()
        .then(|| music_choice(selection.music()));
    QualityReport {
        choices,
        music,
        customised,
        overwritten,
        disposition,
    }
}

/// One audio-format choice as a report — the music equivalent of [`choice`], in format
/// terms rather than resolution. Shared by showing the choice and by setting it.
pub(crate) fn music_choice(format: Format) -> MusicChoice {
    let consequence = format.consequence();
    MusicChoice {
        scope: MUSIC.to_owned(),
        format: format.label().to_owned(),
        means: format.means().to_owned(),
        targets: consequence.format.to_owned(),
        size_per_hour: consequence.size_per_hour.to_owned(),
        note: consequence.note.to_owned(),
    }
}

/// One preset in force, with its consequence and whether this host would have to
/// transcode it in software.
fn choice(scope: &str, preset: Preset, playback: Playback) -> PresetChoice {
    let consequence = preset.consequence();
    PresetChoice {
        scope: scope.to_owned(),
        preset: preset.label().to_owned(),
        means: preset.means().to_owned(),
        resolution: consequence.resolution.to_owned(),
        size_per_hour: consequence.size_per_hour.to_owned(),
        transcoding: consequence.transcoding.to_owned(),
        needs_transcoding_here: warn_before_confirming(preset, playback).is_some(),
    }
}

#[cfg(test)]
mod tests;
