use std::path::PathBuf;

use include_dir::{include_dir, Dir};

use super::{quality, straining, Ctx, QualityAction};
use crate::config::{store, Settings, JELLYFIN_MODE_KEY};
use crate::model::{Disposition, PresetChoice, QualityReport};
use crate::platform::Environment;
use crate::quality::Preset;
use crate::stack::Source;
use crate::test_support::a_context;

static STACKLET: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/stacklet");

/// A scratch env-file path unique to this process and test, its directory
/// cleared so a run starts from nothing.
fn scratch(name: &str) -> lemonfiber_fixtures::scratch::Scratch {
    lemonfiber_fixtures::scratch::Scratch::unmade(name).within(".env")
}

/// A context whose only wired-up parts are the environment file and platform;
/// the quality handler reaches nothing else.
fn ctx(env: Option<PathBuf>, environment: Environment) -> Ctx {
    a_context()
        .settings(Settings {
            env_file: env,
            ..Settings::default()
        })
        .environment(environment)
        .build()
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
    let context = ctx(Some(env.to_path_buf()), Environment::LinuxNative);

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
    let context = ctx(Some(env.to_path_buf()), Environment::LinuxNative);

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
fn a_chosen_music_format_is_shown_apart_from_the_resolution_presets() {
    use crate::audio::Format;
    use crate::quality::Selection;
    let env = scratch("music-show");
    let mut selection = Selection::everywhere(Preset::default_preset());
    selection.set_music(Format::HiRes);
    assert!(selection.music_chosen());
    assert_eq!(selection.music(), Format::HiRes);
    let _ = store::write(
        &env.with_file_name("quality.json"),
        &serde_json::to_string(&selection).unwrap_or_default(),
    );

    let report = run(
        &ctx(Some(env.to_path_buf()), Environment::LinuxNative),
        QualityAction::Show,
    );
    let music = report.music.unwrap_or_default();
    assert_eq!(music.scope, "music");
    assert_eq!(music.format, Format::HiRes.label());
    assert!(!music.means.is_empty());
    assert!(!music.targets.is_empty());
}

#[test]
fn showing_with_no_music_chosen_reports_none() {
    // The default axis stands in for playback, but music is only reported once the
    // operator has actually chosen a format — otherwise it is absent, not defaulted.
    let report = run(&ctx(None, Environment::LinuxNative), QualityAction::Show);
    assert!(report.music.is_none());
}

#[test]
fn a_transcoding_choice_is_held_on_a_software_only_host() {
    let env = scratch("held");
    // A Docker Jellyfin on macOS cannot hardware-transcode.
    let _ = store::set(&env, JELLYFIN_MODE_KEY, "docker");
    let context = ctx(Some(env.to_path_buf()), Environment::MacOs);

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
    let context = ctx(Some(env.to_path_buf()), Environment::MacOs);

    let report = run(&context, set(Preset::Maximum, None, true));
    assert_eq!(report.disposition, Disposition::Recorded);
    assert!(global(&report).needs_transcoding_here);
}

/// The choice already on record is weighed again, for a surface that shows the
/// caution long after the confirmation is forgotten.
///
/// A per-type exception is the case worth driving: the global preset asks for
/// nothing, and film night still meets the transcode.
#[test]
fn a_recorded_choice_that_would_be_transcoded_is_still_strained_afterwards() {
    let env = scratch("straining");
    let _ = store::set(&env, JELLYFIN_MODE_KEY, "docker");
    let context = ctx(Some(env.to_path_buf()), Environment::MacOs);
    assert!(
        straining(&context).is_none(),
        "nothing is chosen yet, so nothing is strained"
    );

    let recorded = run(&context, set(Preset::Maximum, Some("movies"), true));
    assert_eq!(recorded.disposition, Disposition::Recorded);

    assert_eq!(
        straining(&context).map(|warning| warning.preset),
        Some(Preset::Maximum),
        "the most demanding preset in force is what playback meets"
    );
}

/// A host that reaches its encoder is strained by nothing, whatever is chosen.
#[test]
fn a_host_that_transcodes_in_hardware_is_never_strained() {
    let env = scratch("unstrained");
    let _ = store::set(&env, JELLYFIN_MODE_KEY, "docker");
    let context = ctx(Some(env.to_path_buf()), Environment::LinuxNative);

    let recorded = run(&context, set(Preset::Maximum, None, false));
    assert_eq!(recorded.disposition, Disposition::Recorded);
    assert!(straining(&context).is_none());
}

/// A machine with nothing configured is answered rather than refused.
#[test]
fn a_machine_with_no_choice_and_no_stack_is_strained_by_nothing() {
    assert!(straining(&ctx(None, Environment::MacOs)).is_none());
}

#[test]
fn native_jellyfin_lets_a_transcoding_choice_through_unheld() {
    let env = scratch("native");
    let _ = store::set(&env, JELLYFIN_MODE_KEY, "native");
    let context = ctx(Some(env.to_path_buf()), Environment::MacOs);

    let report = run(&context, set(Preset::Maximum, None, false));
    assert_eq!(report.disposition, Disposition::Recorded);
    assert!(!global(&report).needs_transcoding_here);
}

#[test]
fn a_rehearsed_set_reports_what_it_would_do_and_writes_nothing() {
    let env = scratch("rehearsed");
    let context = ctx(Some(env.to_path_buf()), Environment::LinuxNative).rehearsing();

    let report = run(&context, set(Preset::SpaceSaving, None, false));
    assert_eq!(report.disposition, Disposition::Rehearsed);
    // Nothing was written: a real show sees the default.
    let shown = run(
        &ctx(Some(env.to_path_buf()), Environment::LinuxNative),
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
    let context = ctx(Some(env.to_path_buf()), Environment::LinuxNative);

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
        &ctx(Some(env.to_path_buf()), Environment::LinuxNative),
        QualityAction::Show
    )
    .is_err());
}

#[test]
fn the_projection_reads_the_recorded_choice() {
    let env = scratch("projection-recorded");
    let context = ctx(Some(env.to_path_buf()), Environment::LinuxNative);
    let _ = quality(&context, set(Preset::Maximum, None, true));
    assert_eq!(super::most_demanding_or_default(&context), Preset::Maximum);
}

#[test]
fn the_projection_falls_back_to_the_default_on_an_unreadable_choice() {
    // A corrupt choice must not fail a diagnostic run: the projection quietly
    // uses the default rather than surfacing, since it writes nothing.
    let env = scratch("projection-corrupt");
    let _ = store::write(&env.with_file_name("quality.json"), "not json");
    let context = ctx(Some(env.to_path_buf()), Environment::LinuxNative);
    assert_eq!(
        super::most_demanding_or_default(&context),
        Preset::default_preset()
    );
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
    let dir = lemonfiber_fixtures::scratch::Scratch::named("quality-ro");
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

/// A context operating the embedded fixture stack, materialised under `into`.
fn embedded_ctx(env: Option<PathBuf>, into: Option<PathBuf>) -> Ctx {
    a_context()
        .over(Source::Embedded(&STACKLET))
        .settings(Settings {
            env_file: env,
            stack_dir: into,
            ..Settings::default()
        })
        .environment(Environment::LinuxNative)
        .build()
}

#[test]
fn reapply_on_an_external_stack_changes_nothing() {
    // An external stack is the operator's; there is nothing lemonfiber wrote to
    // re-assert, so a reapply reports itself and touches nothing.
    let report = run(&ctx(None, Environment::LinuxNative), QualityAction::Reapply);
    assert_eq!(report.disposition, Disposition::Reapplied);
    assert!(!report.customised);
}

#[test]
fn a_rehearsed_reapply_reports_it_would_reapply() {
    let context = ctx(None, Environment::LinuxNative).rehearsing();
    let report = run(&context, QualityAction::Reapply);
    assert_eq!(report.disposition, Disposition::WouldReapply);
}

#[test]
fn reapply_overwrites_a_customised_config_through_the_command() {
    let env = scratch("reapply-overwrite");
    let into = env.with_file_name("stack");
    let context = embedded_ctx(Some(env.to_path_buf()), Some(into.clone()));

    // Choose a preset and bring it onto disk, then hand-edit the config.
    let _ = quality(&context, set(Preset::Maximum, None, true));
    let recyclarr = into.join("config/recyclarr/recyclarr.yml");
    // The set records the choice but does not materialise; bring it on disk via a
    // reapply, then the operator edits it by hand.
    let _ = quality(&context, QualityAction::Reapply);
    let _ = std::fs::write(&recyclarr, "# mine\n");

    // The command sees the customisation and, being the explicit consent,
    // overwrites it with the recorded preset.
    let report = run(&context, QualityAction::Reapply);
    assert_eq!(report.disposition, Disposition::Reapplied);
    assert!(report.customised, "it reports that it overwrote an edit");
    // And says which lines went, rather than only that some did.
    let lost = report
        .overwritten
        .as_ref()
        .map(|edit| edit.diff.clone())
        .unwrap_or_default();
    assert!(lost.contains("- # mine"), "{lost}");
    assert!(std::fs::read_to_string(&recyclarr)
        .unwrap_or_default()
        .contains("sonarr-web-2160p.yml"));
}

/// Nothing of the operator's to lose is nothing to show, which keeps the word and
/// the diff from coming apart.
#[test]
fn a_reapply_over_a_config_nobody_edited_shows_no_diff() {
    let env = scratch("reapply-unedited");
    let into = env.with_file_name("stack");
    let context = embedded_ctx(Some(env.to_path_buf()), Some(into));

    let _ = quality(&context, set(Preset::Maximum, None, true));
    let _ = quality(&context, QualityAction::Reapply);

    let report = run(&context, QualityAction::Reapply);
    assert!(!report.customised);
    assert!(report.overwritten.is_none(), "{:?}", report.overwritten);
}

#[test]
fn a_reapply_with_nowhere_to_write_is_reported() {
    // The embedded stack has a Recyclarr config to re-assert, but no directory it
    // is materialised into, so the reapply is refused rather than guessing.
    let refused = quality(
        &embedded_ctx(Some(scratch("reapply-nowhere").to_path_buf()), None),
        QualityAction::Reapply,
    );
    assert!(
        refused.is_err(),
        "a reapply with nowhere to write is refused"
    );
}
