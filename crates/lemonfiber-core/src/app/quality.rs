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

use std::path::PathBuf;

use super::{Ctx, QualityAction};
use crate::config::{store, JELLYFIN_MODE_KEY};
use crate::error::{Diagnose, Problem};
use crate::model::{Disposition, PresetChoice, QualityReport};
use crate::quality::{Preset, Selection};
use crate::transcoding::{warn_before_confirming, Playback};
use crate::wizard::Library;

/// The scope a global choice is reported under, as opposed to a media type's name.
const EVERYTHING: &str = "everything";

/// Show or change the quality preset.
///
/// A rehearsal reports the choice it would record without writing it, and a choice
/// this host would have to transcode in software is held rather than recorded
/// unless the operator confirmed it — so `--dry-run` and the confirmation both mean
/// here what they mean elsewhere.
pub(super) fn quality(ctx: &Ctx, action: QualityAction) -> Result<QualityReport, Box<Problem>> {
    let mut selection = load_selection(ctx)?;
    let playback = playback(ctx);

    let disposition = match action {
        QualityAction::Show => Disposition::Shown,
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
            if warn_before_confirming(preset, playback).is_some() && !confirm {
                Disposition::Held
            } else if ctx.dry_run {
                Disposition::Rehearsed
            } else {
                save_selection(ctx, &selection)?;
                Disposition::Recorded
            }
        }
    };

    Ok(report(&selection, playback, disposition))
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
pub(super) fn load_selection(ctx: &Ctx) -> Result<Selection, Box<Problem>> {
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

/// Record the choice where the next run — and a backup — will find it.
///
/// Unlike the baseline seeding keeps best-effort, this is the operator's explicit
/// action, so a place it cannot be written is reported rather than swallowed.
fn save_selection(ctx: &Ctx, selection: &Selection) -> Result<(), Box<Problem>> {
    let path = quality_path(ctx).ok_or_else(|| Box::new(store::Failure::Nowhere.problem()))?;
    store::write(&path, &serde_json::to_string(selection).unwrap_or_default())
        .map_err(|failure| Box::new(failure.problem()))
}

/// Where the choice is kept: beside the environment file, in the configuration
/// directory a backup captures, or nowhere when nothing is configured. Derived from
/// the one path the context carries, and equal to
/// [`crate::config::paths::Paths::quality`].
fn quality_path(ctx: &Ctx) -> Option<PathBuf> {
    ctx.settings
        .env_file
        .as_deref()
        .map(|env| env.with_file_name("quality.json"))
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
fn report(selection: &Selection, playback: Playback, disposition: Disposition) -> QualityReport {
    let mut choices = vec![choice(EVERYTHING, selection.global(), playback)];
    for (media_type, preset) in selection.overrides() {
        choices.push(choice(media_type, preset, playback));
    }
    QualityReport {
        choices,
        disposition,
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
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use super::{quality, Ctx, QualityAction};
    use crate::config::{store, Settings, JELLYFIN_MODE_KEY};
    use crate::model::{Disposition, PresetChoice, QualityReport};
    use crate::platform::Environment;
    use crate::quality::Preset;
    use crate::test_support::{spoke, stack, Reporting, Scripted};

    /// A scratch env-file path unique to this process and test, its directory
    /// cleared so a run starts from nothing.
    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-quality-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join(".env")
    }

    /// A context whose only wired-up parts are the environment file and platform;
    /// the quality handler reaches nothing else.
    fn ctx(env: Option<PathBuf>, environment: Environment) -> Ctx {
        Ctx::new(
            Arc::new(Scripted(Ok(spoke("")))),
            Arc::new(Reporting::absent()),
            Arc::new(crate::adapters::System),
            Arc::new(crate::adapters::Disk),
            stack(),
            Settings {
                env_file: env,
                ..Settings::default()
            },
            environment,
        )
    }

    fn set(preset: Preset, media_type: Option<&str>, confirm: bool) -> QualityAction {
        QualityAction::Set {
            preset,
            media_type: media_type.map(ToOwned::to_owned),
            confirm,
        }
    }

    /// Run a command, taking a defaulted (empty) report where it errored — the
    /// error cases assert on the `Err` directly, so a default here fails an
    /// assertion rather than hiding a fault.
    fn run(context: &Ctx, action: QualityAction) -> QualityReport {
        quality(context, action).ok().unwrap_or_default()
    }

    /// The global choice — the first row of a report.
    fn global(report: &QualityReport) -> PresetChoice {
        report.choices.first().cloned().unwrap_or_default()
    }

    #[test]
    fn showing_with_nothing_chosen_offers_the_default() {
        let report = run(&ctx(None, Environment::LinuxNative), QualityAction::Show);
        assert_eq!(report.disposition, Disposition::Shown);
        assert_eq!(report.choices.len(), 1);
        let global = global(&report);
        assert_eq!(global.scope, "everything");
        assert_eq!(global.preset, Preset::default_preset().label());
        assert!(!global.means.is_empty());
        assert!(!global.needs_transcoding_here);
    }

    #[test]
    fn a_chosen_preset_is_recorded_and_reads_back() {
        let env = scratch("recorded");
        let context = ctx(Some(env), Environment::LinuxNative);

        let report = run(&context, set(Preset::HighQuality, None, false));
        assert_eq!(report.disposition, Disposition::Recorded);
        assert_eq!(global(&report).preset, Preset::HighQuality.label());

        // A later show reads the recorded choice rather than the default.
        let shown = run(&context, QualityAction::Show);
        assert_eq!(global(&shown).preset, Preset::HighQuality.label());
    }

    #[test]
    fn a_per_type_choice_is_recorded_apart_from_the_global() {
        let env = scratch("per-type");
        let context = ctx(Some(env), Environment::LinuxNative);

        let report = run(&context, set(Preset::HighQuality, Some("movies"), false));
        assert_eq!(report.disposition, Disposition::Recorded);
        // The global stays the default; movies is set apart.
        assert_eq!(global(&report).scope, "everything");
        assert_eq!(global(&report).preset, Preset::default_preset().label());
        let movies = report
            .choices
            .iter()
            .find(|choice| choice.scope == "movies")
            .map(|choice| choice.preset.as_str());
        assert_eq!(movies, Some(Preset::HighQuality.label()));
    }

    #[test]
    fn a_transcoding_choice_is_held_on_a_software_only_host() {
        let env = scratch("held");
        // A Docker Jellyfin on macOS cannot hardware-transcode.
        let _ = store::set(&env, JELLYFIN_MODE_KEY, "docker");
        let context = ctx(Some(env), Environment::MacOs);

        let report = run(&context, set(Preset::Maximum, None, false));
        assert_eq!(report.disposition, Disposition::Held);
        assert!(global(&report).needs_transcoding_here);
        // Held means not recorded: a show falls back to the default.
        let shown = run(&context, QualityAction::Show);
        assert_eq!(global(&shown).preset, Preset::default_preset().label());
    }

    #[test]
    fn a_confirmed_transcoding_choice_is_recorded() {
        let env = scratch("confirmed");
        let _ = store::set(&env, JELLYFIN_MODE_KEY, "docker");
        let context = ctx(Some(env), Environment::MacOs);

        let report = run(&context, set(Preset::Maximum, None, true));
        assert_eq!(report.disposition, Disposition::Recorded);
        assert!(global(&report).needs_transcoding_here);
    }

    #[test]
    fn native_jellyfin_lets_a_transcoding_choice_through_unheld() {
        let env = scratch("native");
        let _ = store::set(&env, JELLYFIN_MODE_KEY, "native");
        let context = ctx(Some(env), Environment::MacOs);

        let report = run(&context, set(Preset::Maximum, None, false));
        assert_eq!(report.disposition, Disposition::Recorded);
        assert!(!global(&report).needs_transcoding_here);
    }

    #[test]
    fn a_rehearsed_set_reports_what_it_would_do_and_writes_nothing() {
        let env = scratch("rehearsed");
        let context = ctx(Some(env.clone()), Environment::LinuxNative).rehearsing();

        let report = run(&context, set(Preset::SpaceSaving, None, false));
        assert_eq!(report.disposition, Disposition::Rehearsed);
        // Nothing was written: a real show sees the default.
        let shown = run(
            &ctx(Some(env), Environment::LinuxNative),
            QualityAction::Show,
        );
        assert_eq!(global(&shown).preset, Preset::default_preset().label());
    }

    #[test]
    fn an_unparsable_choice_is_surfaced_and_never_overwritten() {
        let env = scratch("corrupt");
        // A present-but-corrupt quality.json: not a first run, and not a value to
        // guess past.
        let corrupt = env.with_file_name("quality.json");
        let _ = store::write(&corrupt, "this is not json");
        let context = ctx(Some(env), Environment::LinuxNative);

        // Both showing and setting refuse rather than reading it as the default.
        assert!(quality(&context, QualityAction::Show).is_err());
        assert!(quality(&context, set(Preset::Maximum, None, true)).is_err());
        // The corrupt file is left exactly as it was — a set did not overwrite it.
        let after = std::fs::read_to_string(&corrupt).unwrap_or_default();
        assert_eq!(after, "this is not json");
    }

    #[test]
    fn a_choice_file_that_cannot_be_read_is_surfaced() {
        // A directory where the file should be: reading it fails with something
        // other than "not found", which must be surfaced, not read as a first run.
        let env = scratch("unreadable");
        let as_dir = env.with_file_name("quality.json");
        let _ = std::fs::create_dir_all(&as_dir);

        assert!(quality(
            &ctx(Some(env), Environment::LinuxNative),
            QualityAction::Show
        )
        .is_err());
    }

    #[test]
    fn a_set_with_nowhere_to_record_is_refused() {
        let refused = quality(
            &ctx(None, Environment::LinuxNative),
            set(Preset::Balanced, None, false),
        );
        assert!(refused.is_err(), "a set with no config path cannot record");
    }

    #[cfg(unix)]
    #[test]
    fn a_set_that_cannot_be_written_is_reported() {
        use std::os::unix::fs::PermissionsExt as _;

        // A directory that can be read (so an absent choice loads as the default)
        // but not written (so recording the choice fails). The write is surfaced,
        // not swallowed — this is the operator's explicit action.
        let dir =
            std::env::temp_dir().join(format!("lemonfiber-quality-ro-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500));

        let refused = quality(
            &ctx(Some(dir.join(".env")), Environment::LinuxNative),
            set(Preset::Balanced, None, false),
        );
        assert!(refused.is_err(), "an unwritable location is reported");

        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
