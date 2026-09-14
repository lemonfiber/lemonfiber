//! Writing the embedded stack to disk without clobbering an operator's edits.
//!
//! The stack lemonfiber ships is written out so Compose can read it, on every
//! invocation rather than cached, so an upgrade never leaves a stale copy behind.
//! Those files are the operator's to edit, though, so a blind re-write would lose a
//! change they made by hand on the next run. This writes each file only where it is
//! safe to — absent, or still holding what lemonfiber last wrote — and leaves a file
//! the operator has changed exactly as it is, reporting it with a diff rather than
//! overwriting it. An external stack is theirs entirely and is left untouched.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use crate::config::store;
use crate::materialised::{checksum, decide, diff, Decision, Materialised};
use crate::model::StackEdit;
use crate::quality::Selection;
use crate::stack::{Failure, Source};

/// The stack file the quality choice is carried into: Recyclarr's config, whose
/// `include:` template lists a preset rewrites.
///
/// Matched against a file's key, which is its embedded path — baked with forward
/// slashes on every platform by the stack embedding, the same separator the
/// materialised record keys already use. It would need revisiting only if the
/// stack ever came from a real filesystem walk on a back-slash OS.
const RECYCLARR_CONFIG: &str = "config/recyclarr/recyclarr.yml";

/// Write the stack under `into`, preserving any file the operator has edited, and
/// return where it lives and which files were left as they set them.
///
/// An external stack is already on disk and is returned as it is. An embedded stack
/// is written file by file: each is compared, by content, against what lemonfiber
/// last wrote (read from `record_path`) so an operator's edit is told from a version
/// not yet upgraded — the edit is preserved and reported, the rest written. The
/// record of what was written is updated for the next run.
///
/// When a `selection` is given, it is carried into the stack as it is written: the
/// Recyclarr config's template lists are rewritten to the chosen preset, so what
/// lemonfiber materialises already reflects the choice. The default selection
/// rewrites the shipped config to itself, so an unconfigured stack is untouched.
/// With no selection — a teardown, a restart, a rehearsal — the Recyclarr config
/// is left exactly as it is on disk rather than being written back to the shipped
/// default, so an already-applied preset is never reverted by a command that has
/// no business changing it.
///
/// # Errors
///
/// Returns [`Failure`] when there is nowhere to write an embedded stack to, or when
/// a file cannot be written.
pub(super) fn materialise(
    source: Source,
    into: Option<&Path>,
    record_path: Option<&Path>,
    selection: Option<&Selection>,
    unmanaged: &[(String, String)],
) -> Result<(PathBuf, Vec<StackEdit>), Failure> {
    write_stack(
        source,
        into,
        record_path,
        selection,
        unmanaged,
        Pass::Materialise,
    )
}

/// Overwrite the stack back to lemonfiber's own version, reverting every file the
/// operator had edited — the write side of a full reset. Returns where the stack lives
/// and, as [`StackEdit`]s, the edits that were reverted (the diff of what was lost). A
/// reset is the explicit consent to let lemonfiber's state win, so the record is brought
/// to what was written and the reverted file is no longer read as drift.
///
/// # Errors
///
/// Returns [`Failure`] when there is nowhere to write to, or a file cannot be written.
pub(super) fn reset_stack(
    source: Source,
    into: Option<&Path>,
    record_path: Option<&Path>,
    selection: Option<&Selection>,
    unmanaged: &[(String, String)],
) -> Result<(PathBuf, Vec<StackEdit>), Failure> {
    write_stack(source, into, record_path, selection, unmanaged, Pass::Reset)
}

/// Where the stack would live and which files the operator has edited, without
/// writing a byte of it — the materialise a rehearsal does.
///
/// The same walk over the same files, with the writing left out. That is the whole of
/// the difference, and it is why this is a `Pass` rather than a second function: the
/// edits a rehearsal reports are found by the comparison an ordinary run makes on its
/// way to writing, so a preview computed some other way would be a second account of
/// what is on disk and the two would drift.
///
/// # Errors
///
/// Returns [`Failure`] where there is nowhere the stack could live to read from.
pub(super) fn would_materialise(
    source: Source,
    into: Option<&Path>,
    record_path: Option<&Path>,
    selection: Option<&Selection>,
    unmanaged: &[(String, String)],
) -> Result<(PathBuf, Vec<StackEdit>), Failure> {
    write_stack(
        source,
        into,
        record_path,
        selection,
        unmanaged,
        Pass::Preview,
    )
}

/// The edits a reset would revert, without touching a thing — the operator's hand-edited
/// stack files, each with the diff of what would be lost. The preview a reset shows before
/// it is confirmed.
///
/// # Errors
///
/// Returns [`Failure`] only where there is nowhere the stack could live to read from.
pub(super) fn pending_reverts(
    source: Source,
    into: Option<&Path>,
    record_path: Option<&Path>,
    selection: Option<&Selection>,
    unmanaged: &[(String, String)],
) -> Result<Vec<StackEdit>, Failure> {
    write_stack(
        source,
        into,
        record_path,
        selection,
        unmanaged,
        Pass::Preview,
    )
    .map(|(_, edits)| edits)
}

/// Which pass over the stack this is: an ordinary materialise (write lemonfiber's, keep
/// the operator's edits), a reset (write lemonfiber's over the operator's edits too), or a
/// preview (touch nothing, only report what a reset would revert).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pass {
    Materialise,
    Reset,
    Preview,
}

/// The one walk behind materialise, reset and its preview: decide each file three ways,
/// and act per `pass` — writing lemonfiber's version, preserving or reverting an edit, or
/// only collecting what a reset would revert. An operator's edit is always returned with
/// its diff; the pass decides whether it was preserved, reverted, or merely previewed.
fn write_stack(
    source: Source,
    into: Option<&Path>,
    record_path: Option<&Path>,
    selection: Option<&Selection>,
    unmanaged: &[(String, String)],
    pass: Pass,
) -> Result<(PathBuf, Vec<StackEdit>), Failure> {
    let files = source.files();
    if files.is_empty() {
        // External, or nothing to write: left exactly as it is.
        return source.materialise(into).map(|path| (path, Vec::new()));
    }
    let Some(into) = into else {
        return Err(Failure::NowhereToWrite);
    };

    let writing = pass != Pass::Preview;
    let mut record = load(record_path);
    let mut edits = Vec::new();
    for (relative, content) in files {
        let key = relative.to_string_lossy();
        // An area the operator declared unmanaged, skipped on every pass — including a
        // reset, which is otherwise the consent to let lemonfiber's state win. Two
        // explicit decisions, and the one that says "never write here" is the one that
        // has to hold, because the other failing quietly is how somebody comes to
        // believe a file is theirs while a command they ran reverts it.
        //
        // Skipped before the comparison rather than after it, so the record of what
        // lemonfiber wrote gains no entry for a file lemonfiber is not writing. An
        // entry there would read, on the first run after a declaration is taken back,
        // as lemonfiber's own file to overwrite.
        if crate::unmanaged::covers(unmanaged, &key) {
            continue;
        }
        let content = match selection {
            Some(selection) => carrying_the_choice(&key, content, selection),
            // No choice to carry, and the Recyclarr config left as it is rather than
            // written back to the shipped default — which would revert a preset.
            None if key == RECYCLARR_CONFIG => continue,
            None => Cow::Borrowed(content),
        };
        let target = into.join(&relative);
        let desired = checksum(&content);
        let on_disk = std::fs::read(&target).ok();
        let actual = on_disk.as_deref().map(checksum);
        match decide(record.checksum(&key), actual, desired) {
            Decision::Write if writing => {
                write(&target, &content)?;
                record.record(&key, desired);
            }
            // Already what lemonfiber would write: recorded so the next run reads it as
            // lemonfiber's own rather than the operator's.
            Decision::Fresh if writing => record.record(&key, desired),
            // A preview touches nothing and records nothing.
            Decision::Write | Decision::Fresh => {}
            // The operator's edit. An ordinary materialise leaves it, its record kept at
            // what lemonfiber last wrote so it goes on being recognised; a reset writes
            // lemonfiber's version over it and records that; a preview does neither.
            // Either way it is returned with a diff — held back, reverted, or previewed.
            Decision::Preserve => {
                let yours = on_disk.unwrap_or_default();
                if pass == Pass::Reset {
                    write(&target, &content)?;
                    record.record(&key, desired);
                }
                edits.push(StackEdit {
                    path: key.into_owned(),
                    diff: diff(
                        &String::from_utf8_lossy(&yours),
                        &String::from_utf8_lossy(&content),
                    ),
                });
            }
        }
    }
    if writing {
        save(record_path, &record);
    }
    Ok((into.to_path_buf(), edits))
}

/// The content lemonfiber intends for a stack file, given the quality choice: the
/// Recyclarr config with its template lists rewritten to the preset, every other
/// file exactly as it ships. The rewrite is what makes the choice take effect on
/// the next stack write, and the default selection returns the shipped config
/// unchanged, so a stack no one has chosen a preset for is materialised as before.
fn carrying_the_choice<'a>(key: &str, content: &'a [u8], selection: &Selection) -> Cow<'a, [u8]> {
    if key == RECYCLARR_CONFIG {
        let rewritten = crate::recyclarr::rewrite(&String::from_utf8_lossy(content), selection);
        Cow::Owned(rewritten.into_bytes())
    } else {
        Cow::Borrowed(content)
    }
}

/// Whether the materialised Recyclarr config has been hand-edited since lemonfiber
/// last wrote it — the `customised` state, in which a preset is no longer
/// authoritative because the operator has tuned the config by hand.
///
/// False where there is nothing to judge against: no record of what lemonfiber
/// wrote, or no config on disk. It is the same comparison [`decide`] makes — on-disk
/// against the record — read without writing anything.
pub(super) fn recyclarr_customised(into: Option<&Path>, record_path: Option<&Path>) -> bool {
    let Some(into) = into else {
        return false;
    };
    let Some(recorded) = load(record_path).checksum(RECYCLARR_CONFIG) else {
        return false;
    };
    match std::fs::read(into.join(RECYCLARR_CONFIG)) {
        Ok(bytes) => checksum(&bytes) != recorded,
        Err(_) => false,
    }
}

/// Re-assert the recorded preset over the Recyclarr config, overwriting a hand-edit
/// where an ordinary run would have preserved it — the operator's explicit consent
/// to let the preset win. Records the new content so it is recognised as lemonfiber's
/// own again.
///
/// Returns the edit it replaced, with the diff of what was lost against what was
/// written, or nothing where the config was already in lemonfiber's own hand. A bare
/// yes-or-no was what this answered for a long while, and consent given against a
/// yes-or-no is consent to something the operator was never shown: they know a file
/// they edited is about to go and not which of their lines is in it. The diff is
/// masked the way every other diff of a stack file is, so a credential that drifted
/// is named without either value being printed.
///
/// A rehearsal reports what it would replace and writes nothing. An external stack,
/// which lemonfiber does not materialise, is left untouched.
///
/// # Errors
///
/// Returns [`Failure`] when there is nowhere to write, or the config cannot be written.
pub(super) fn reapply_recyclarr(
    source: Source,
    into: Option<&Path>,
    record_path: Option<&Path>,
    selection: &Selection,
    unmanaged: &[(String, String)],
    rehearse: bool,
) -> Result<Option<StackEdit>, Failure> {
    // The one command whose whole purpose is to overwrite an operator's edit, held by
    // the one declaration whose whole purpose is to stop that. Answered as "nothing was
    // replaced", which is true: the config is theirs and stays exactly as it is.
    if crate::unmanaged::covers(unmanaged, RECYCLARR_CONFIG) {
        return Ok(None);
    }
    let Some(shipped) = shipped_recyclarr(source) else {
        // External, or a stack with no Recyclarr config: nothing lemonfiber manages.
        // This is also the guard that keeps an external stack safe — `into` is the
        // built-in stack directory, not where an external stack lives, so writing
        // there would be wrong. An external source has no embedded files, so it
        // returns here before touching `into`; that invariant is load-bearing.
        return Ok(None);
    };
    let Some(into) = into else {
        return Err(Failure::NowhereToWrite);
    };
    let target = into.join(RECYCLARR_CONFIG);
    let desired = crate::recyclarr::rewrite(&String::from_utf8_lossy(shipped), selection);

    // Read before the write, and read once. The same comparison `recyclarr_customised`
    // makes — a record of what lemonfiber wrote, and a file on disk that no longer
    // matches it — but holding the content rather than the verdict, because what is
    // about to be overwritten cannot be read back afterwards.
    let recorded = load(record_path).checksum(RECYCLARR_CONFIG);
    let theirs = std::fs::read(&target)
        .ok()
        .filter(|bytes| recorded.is_some_and(|was| was != checksum(bytes)));

    if !rehearse {
        write(&target, desired.as_bytes())?;
        let mut record = load(record_path);
        record.record(RECYCLARR_CONFIG, checksum(desired.as_bytes()));
        save(record_path, &record);
    }
    Ok(theirs.map(|yours| StackEdit {
        path: RECYCLARR_CONFIG.to_owned(),
        diff: diff(&String::from_utf8_lossy(&yours), &desired),
    }))
}

/// The Recyclarr config this stack ships, or `None` for a stack that has none — an
/// external stack lemonfiber does not write, or one without the file.
fn shipped_recyclarr(source: Source) -> Option<&'static [u8]> {
    source
        .files()
        .into_iter()
        .find(|(relative, _)| relative.to_string_lossy() == RECYCLARR_CONFIG)
        .map(|(_, content)| content)
}

/// Read the record of what lemonfiber last wrote, or an empty one where none is
/// kept. A record that cannot be read leaves an empty one, under which a file that
/// differs is preserved rather than overwritten — the safe direction when what was
/// written cannot be recalled.
fn load(record_path: Option<&Path>) -> Materialised {
    super::record::kept(record_path)
}

/// Write the record where the next run will read it. Best-effort, like the stack it
/// describes: a run that cannot persist it still wrote the stack, and the worst a
/// lost record costs is the next run preserving a file it could have safely rewritten.
fn save(record_path: Option<&Path>, record: &Materialised) {
    if let Some(path) = record_path {
        let _ = store::write(path, &serde_json::to_string(record).unwrap_or_default());
    }
}

/// Write one stack file, making its parent directory first. Both ways it can fail —
/// the directory or the file — funnel through one place that names the file, so
/// there is a single wording of the failure rather than one per step.
fn write(target: &Path, content: &[u8]) -> Result<(), Failure> {
    write_file(target, content).map_err(|error| Failure::NotWritten {
        path: target.to_path_buf(),
        reason: error.to_string(),
    })
}

/// The filesystem writes themselves: the parent directory, then the file. The
/// parent is made with a statement-level `?` rather than one inside an `if`, which a
/// coverage pass reads as a branch that the always-present parent never leaves.
fn write_file(target: &Path, content: &[u8]) -> std::io::Result<()> {
    target.parent().map_or(Ok(()), std::fs::create_dir_all)?;
    std::fs::write(target, content)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use include_dir::{include_dir, Dir};

    use super::{
        materialise, pending_reverts, reapply_recyclarr, recyclarr_customised, reset_stack,
    };
    use crate::quality::{Preset, Selection};
    use crate::stack::{Failure, Source};

    static STACKLET: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/stacklet");

    /// A clean scratch directory for one test, and the record path beside it.
    fn scratch(name: &str) -> (PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!("lemonfiber-mat-{name}"));
        let _ = std::fs::remove_dir_all(&root);
        (root.join("stack"), root.join("materialised.json"))
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap_or_default()
    }

    /// The default choice, which rewrites the shipped Recyclarr config to itself —
    /// the selection the file-writing tests use, since it changes nothing.
    fn balanced() -> Selection {
        Selection::everywhere(Preset::Balanced)
    }

    #[test]
    fn every_file_is_written_and_recorded_then_left_on_a_second_run() {
        let (into, record) = scratch("write-and-leave");
        let source = Source::Embedded(&STACKLET);

        let first = materialise(source, Some(&into), Some(&record), Some(&balanced()), &[]);
        let (path, edits) = first.unwrap_or((PathBuf::new(), Vec::new()));
        assert_eq!(path, into);
        assert!(edits.is_empty(), "a fresh materialise reports no edits");
        // Every file, including the nested one, is written with the embedded content.
        assert!(read(&into.join("compose.yaml")).contains("image: sonarr"));
        assert!(read(&into.join("stack.toml")).contains("stacklet"));
        assert!(read(&into.join("fragments/tv.yaml")).contains("profiles"));
        assert!(
            read(&record).contains("compose.yaml"),
            "the write is recorded"
        );

        // A second run finds every file exactly as it left it: nothing to report.
        let (_, again) = materialise(source, Some(&into), Some(&record), Some(&balanced()), &[])
            .unwrap_or((PathBuf::new(), Vec::new()));
        assert!(again.is_empty(), "an unchanged file is left, not reported");
    }

    #[test]
    fn an_edited_file_is_preserved_and_reported_with_a_diff() {
        let (into, record) = scratch("preserve-edit");
        let source = Source::Embedded(&STACKLET);
        let _ = materialise(source, Some(&into), Some(&record), Some(&balanced()), &[]);

        // The operator edits a materialised file by hand.
        let edited = "services:\n  sonarr:\n    image: my-own-sonarr\n";
        let _ = std::fs::write(into.join("compose.yaml"), edited);

        let (_, edits) = materialise(source, Some(&into), Some(&record), Some(&balanced()), &[])
            .unwrap_or((PathBuf::new(), Vec::new()));
        assert_eq!(edits.len(), 1, "only the edited file is reported");
        let edit = edits.first();
        assert!(edit.is_some_and(|edit| edit.path == "compose.yaml"));
        assert!(
            edit.is_some_and(|edit| edit.diff.contains("- ") && edit.diff.contains("+ ")),
            "the diff shows both sides",
        );
        // The operator's edit is left exactly as they made it, not overwritten.
        assert_eq!(read(&into.join("compose.yaml")), edited);
    }

    #[test]
    fn a_reset_reverts_an_edited_file_to_lemonfibers_and_names_it() {
        let (into, record) = scratch("reset-revert");
        let source = Source::Embedded(&STACKLET);
        let (_, _) = materialise(source, Some(&into), Some(&record), Some(&balanced()), &[])
            .unwrap_or((PathBuf::new(), Vec::new()));
        let shipped = read(&into.join("compose.yaml"));

        // The operator edits a file, then resets.
        let edited = "services:\n  sonarr:\n    image: my-own-sonarr\n";
        let _ = std::fs::write(into.join("compose.yaml"), edited);

        let (_, reverted) = reset_stack(source, Some(&into), Some(&record), Some(&balanced()), &[])
            .unwrap_or((PathBuf::new(), Vec::new()));
        assert_eq!(reverted.len(), 1, "the reverted edit is named");
        assert!(reverted
            .first()
            .is_some_and(|edit| edit.path == "compose.yaml"));
        // The edit is gone: the file is lemonfiber's own again.
        assert_eq!(read(&into.join("compose.yaml")), shipped);
        // And a following materialise sees no drift — the reset re-recorded it.
        let (_, again) = materialise(source, Some(&into), Some(&record), Some(&balanced()), &[])
            .unwrap_or((PathBuf::new(), Vec::new()));
        assert!(
            again.is_empty(),
            "the reverted file is no longer read as drift"
        );
    }

    #[test]
    fn a_preview_names_the_reverts_but_writes_nothing() {
        let (into, record) = scratch("reset-preview");
        let source = Source::Embedded(&STACKLET);
        let _ = materialise(source, Some(&into), Some(&record), Some(&balanced()), &[]);

        let edited = "services:\n  sonarr:\n    image: my-own-sonarr\n";
        let _ = std::fs::write(into.join("compose.yaml"), edited);

        let pending = pending_reverts(source, Some(&into), Some(&record), Some(&balanced()), &[])
            .unwrap_or_default();
        assert_eq!(pending.len(), 1, "the edit that would be reverted is named");
        // The preview touched nothing: the operator's edit is still there.
        assert_eq!(read(&into.join("compose.yaml")), edited);
    }

    #[test]
    fn an_external_stack_is_returned_and_nothing_is_written() {
        let (into, record) = scratch("external");
        let external = Path::new("/some/operator/stack");
        let (path, edits) = materialise(
            Source::External(external),
            Some(&into),
            Some(&record),
            Some(&balanced()),
            &[],
        )
        .unwrap_or((PathBuf::new(), vec![]));
        assert_eq!(path, external, "an external stack is used where it lives");
        assert!(edits.is_empty());
        assert!(!into.exists(), "nothing was written for an external stack");
    }

    #[test]
    fn an_embedded_stack_with_nowhere_to_write_is_refused() {
        let refusal = materialise(
            Source::Embedded(&STACKLET),
            None,
            None,
            Some(&balanced()),
            &[],
        );
        assert!(matches!(refusal, Err(Failure::NowhereToWrite)));
    }

    #[test]
    fn a_file_where_a_directory_must_go_is_a_write_failure() {
        let (into, record) = scratch("blocked");
        // A plain file sits where the stack directory needs to be, so its parent
        // cannot be made and the write fails rather than guessing.
        if let Some(parent) = into.parent() {
            let _ = std::fs::create_dir_all(parent);
            let _ = std::fs::write(&into, "not a directory");
        }
        let failure = materialise(
            Source::Embedded(&STACKLET),
            Some(&into),
            Some(&record),
            Some(&balanced()),
            &[],
        );
        assert!(matches!(failure, Err(Failure::NotWritten { .. })));
    }

    #[test]
    fn without_a_record_path_the_stack_is_still_written() {
        let (into, _) = scratch("no-record");
        let (_, edits) = materialise(
            Source::Embedded(&STACKLET),
            Some(&into),
            None,
            Some(&balanced()),
            &[],
        )
        .unwrap_or((PathBuf::new(), vec![]));
        assert!(edits.is_empty());
        assert!(read(&into.join("stack.toml")).contains("stacklet"));
    }

    #[test]
    fn a_chosen_preset_is_carried_into_the_recyclarr_config() {
        let (into, record) = scratch("recyclarr-maximum");
        let maximum = Selection::everywhere(Preset::Maximum);

        let (_, edits) = materialise(
            Source::Embedded(&STACKLET),
            Some(&into),
            Some(&record),
            Some(&maximum),
            &[],
        )
        .unwrap_or((PathBuf::new(), vec![]));
        // Written, not reported as an edit: this is lemonfiber's own choice landing.
        assert!(edits.is_empty());
        let written = read(&into.join("config/recyclarr/recyclarr.yml"));
        // The 4K includes are in force, and the Balanced ones the fixture shipped
        // with are gone.
        assert!(written.contains("sonarr-web-2160p.yml"));
        assert!(written.contains("radarr-uhd-bluray-web.yml"));
        assert!(!written.contains("sonarr-web-1080p.yml"));
    }

    #[test]
    fn the_default_choice_leaves_the_shipped_recyclarr_config_untouched() {
        let (into, record) = scratch("recyclarr-default");
        let (_, edits) = materialise(
            Source::Embedded(&STACKLET),
            Some(&into),
            Some(&record),
            Some(&balanced()),
            &[],
        )
        .unwrap_or((PathBuf::new(), vec![]));
        assert!(edits.is_empty());
        // The default rewrites the shipped config to itself: byte for byte what the
        // fixture ships, so a stack no one chose a preset for is materialised as before.
        let shipped = include_str!("../../tests/fixtures/stacklet/config/recyclarr/recyclarr.yml");
        assert_eq!(read(&into.join("config/recyclarr/recyclarr.yml")), shipped);
    }

    #[test]
    fn no_selection_writes_the_rest_but_skips_the_recyclarr_config() {
        // A teardown/restart/rehearsal carries no choice: the stack is still
        // written, but the Recyclarr config is left alone rather than written back
        // to the shipped default.
        let (into, record) = scratch("recyclarr-none");
        let (_, edits) = materialise(
            Source::Embedded(&STACKLET),
            Some(&into),
            Some(&record),
            None,
            &[],
        )
        .unwrap_or((PathBuf::new(), vec![]));
        assert!(edits.is_empty());
        assert!(read(&into.join("compose.yaml")).contains("image: sonarr"));
        assert!(
            !into.join("config/recyclarr/recyclarr.yml").exists(),
            "the Recyclarr config is left untouched, not written",
        );
    }

    #[test]
    fn no_selection_does_not_revert_an_applied_preset() {
        let (into, record) = scratch("recyclarr-no-revert");
        // A preset was applied on a prior up.
        let _ = materialise(
            Source::Embedded(&STACKLET),
            Some(&into),
            Some(&record),
            Some(&Selection::everywhere(Preset::Maximum)),
            &[],
        );
        let recyclarr = into.join("config/recyclarr/recyclarr.yml");
        assert!(read(&recyclarr).contains("sonarr-web-2160p.yml"));

        // A later command carrying no choice leaves the applied preset exactly as it
        // is — not written back to the shipped default.
        let _ = materialise(
            Source::Embedded(&STACKLET),
            Some(&into),
            Some(&record),
            None,
            &[],
        );
        assert!(
            read(&recyclarr).contains("sonarr-web-2160p.yml"),
            "the applied preset is not reverted",
        );
    }

    #[test]
    fn a_config_is_customised_only_once_it_differs_from_the_record() {
        let (into, record) = scratch("customised");
        // Nothing written yet: nothing to be customised against.
        assert!(!recyclarr_customised(Some(&into), Some(&record)));

        // Applied and untouched: lemonfiber's own, not customised.
        let _ = materialise(
            Source::Embedded(&STACKLET),
            Some(&into),
            Some(&record),
            Some(&balanced()),
            &[],
        );
        assert!(!recyclarr_customised(Some(&into), Some(&record)));

        // The operator tunes it by hand: now it is customised.
        let recyclarr = into.join("config/recyclarr/recyclarr.yml");
        let _ = std::fs::write(&recyclarr, "# mine\n");
        assert!(recyclarr_customised(Some(&into), Some(&record)));

        // Deleted while the record persists: nothing on disk to be customised, so a
        // reapply would simply write it again.
        let _ = std::fs::remove_file(&recyclarr);
        assert!(!recyclarr_customised(Some(&into), Some(&record)));
    }

    #[test]
    fn reapply_overwrites_a_customised_config_and_records_it() {
        let (into, record) = scratch("reapply");
        let maximum = Selection::everywhere(Preset::Maximum);
        let _ = materialise(
            Source::Embedded(&STACKLET),
            Some(&into),
            Some(&record),
            Some(&maximum),
            &[],
        );
        let recyclarr = into.join("config/recyclarr/recyclarr.yml");
        // The operator hand-edits it.
        let _ = std::fs::write(&recyclarr, "# mine\n");
        assert!(recyclarr_customised(Some(&into), Some(&record)));

        // Reapply re-asserts the recorded preset over the edit.
        let overwritten = reapply_recyclarr(
            Source::Embedded(&STACKLET),
            Some(&into),
            Some(&record),
            &maximum,
            &[],
            false,
        )
        .unwrap_or_default();
        let edit = overwritten.as_ref();
        assert!(edit.is_some(), "an edit was overwritten");
        assert!(edit.is_some_and(|edit| edit.path.ends_with("recyclarr.yml")));
        // The operator is shown which of their lines went, not only that some did.
        assert!(
            edit.is_some_and(|edit| edit.diff.contains("- # mine")),
            "{overwritten:?}"
        );
        assert!(
            edit.is_some_and(|edit| edit.diff.contains('+')),
            "{overwritten:?}"
        );
        assert!(read(&recyclarr).contains("sonarr-web-2160p.yml"));
        // Recorded as lemonfiber's own again: no longer customised.
        assert!(!recyclarr_customised(Some(&into), Some(&record)));
    }

    /// The diff of a replaced config reaches a terminal, its scrollback and any bug
    /// report pasted out of it, and a Recyclarr config carries a key per instance.
    #[test]
    fn a_credential_in_the_config_a_reapply_replaces_is_named_and_never_printed() {
        let (into, record) = scratch("reapply-secret");
        let maximum = Selection::everywhere(Preset::Maximum);
        let _ = materialise(
            Source::Embedded(&STACKLET),
            Some(&into),
            Some(&record),
            Some(&maximum),
            &[],
        );
        let recyclarr = into.join("config/recyclarr/recyclarr.yml");
        let key = ["a", "recyclarr", "key"].join("-");
        let _ = std::fs::write(&recyclarr, format!("    api_key: {key}\n"));

        let overwritten = reapply_recyclarr(
            Source::Embedded(&STACKLET),
            Some(&into),
            Some(&record),
            &maximum,
            &[],
            true,
        )
        .unwrap_or_default();
        let shown = overwritten.map(|edit| edit.diff).unwrap_or_default();

        assert!(shown.contains("api_key"), "{shown}");
        assert!(
            !shown.contains(&key),
            "the key survived into the diff: {shown}"
        );
    }

    #[test]
    fn a_reapply_over_a_config_already_in_lemonfibers_own_hand_replaces_nothing() {
        let (into, record) = scratch("reapply-clean");
        let maximum = Selection::everywhere(Preset::Maximum);
        let _ = materialise(
            Source::Embedded(&STACKLET),
            Some(&into),
            Some(&record),
            Some(&maximum),
            &[],
        );

        let overwritten = reapply_recyclarr(
            Source::Embedded(&STACKLET),
            Some(&into),
            Some(&record),
            &maximum,
            &[],
            false,
        )
        .unwrap_or_default();
        assert!(
            overwritten.is_none(),
            "nothing of the operator's was there to lose: {overwritten:?}"
        );
    }

    #[test]
    fn a_rehearsed_reapply_writes_nothing() {
        let (into, record) = scratch("reapply-dry");
        let maximum = Selection::everywhere(Preset::Maximum);
        let _ = materialise(
            Source::Embedded(&STACKLET),
            Some(&into),
            Some(&record),
            Some(&maximum),
            &[],
        );
        let recyclarr = into.join("config/recyclarr/recyclarr.yml");
        let _ = std::fs::write(&recyclarr, "# mine\n");

        let overwritten = reapply_recyclarr(
            Source::Embedded(&STACKLET),
            Some(&into),
            Some(&record),
            &maximum,
            &[],
            true,
        )
        .unwrap_or_default();
        assert!(
            overwritten.is_some(),
            "the rehearsal reports it would overwrite an edit"
        );
        assert!(
            overwritten.is_some_and(|edit| edit.diff.contains("- # mine")),
            "the rehearsal shows what it would replace"
        );
        // The edit is still on disk: a rehearsal changed nothing.
        assert_eq!(read(&recyclarr), "# mine\n");
    }

    /// A declaration is a name and the reason somebody gave for writing it down.
    fn declared(area: &str) -> Vec<(String, String)> {
        vec![(
            area.to_owned(),
            "my own edits live in this one and I keep them".to_owned(),
        )]
    }

    /// A file beneath a declared name is never written — not on a first materialise,
    /// and not on the run that would have upgraded it.
    #[test]
    fn a_file_beneath_a_declared_area_is_never_written() {
        let (into, record) = scratch("unmanaged-file");
        let source = Source::Embedded(&STACKLET);
        let theirs = declared("config/recyclarr");

        let (_, edits) = materialise(
            source,
            Some(&into),
            Some(&record),
            Some(&balanced()),
            &theirs,
        )
        .unwrap_or((PathBuf::new(), Vec::new()));

        assert!(
            !into.join("config/recyclarr/recyclarr.yml").exists(),
            "a file the operator declared unmanaged was written"
        );
        // And the rest of the stack is written as usual: one declaration is not a
        // refusal to maintain anything else.
        assert!(read(&into.join("compose.yaml")).contains("image: sonarr"));
        // Nor is it reported as an edit held back, which would be reporting drift
        // about an area the operator said is not lemonfiber's to have an opinion on.
        assert!(edits.is_empty(), "{edits:?}");
        // And nothing is recorded for it: a checksum here would read, on the first run
        // after the declaration is taken back, as lemonfiber's own file to overwrite.
        assert!(
            !read(&record).contains("recyclarr"),
            "a file lemonfiber did not write was recorded as though it had"
        );
    }

    /// Two explicit decisions meet here, and the one that says *never write* wins.
    #[test]
    fn a_reset_does_not_revert_a_file_the_operator_declared_unmanaged() {
        let (into, record) = scratch("unmanaged-reset");
        let source = Source::Embedded(&STACKLET);
        let theirs = declared("compose.yaml");
        let _ = materialise(source, Some(&into), Some(&record), Some(&balanced()), &[]);

        let mine = "services:\n  sonarr:\n    image: an-image-of-my-own\n";
        let _ = std::fs::write(into.join("compose.yaml"), mine);

        let (_, reverted) = reset_stack(
            source,
            Some(&into),
            Some(&record),
            Some(&balanced()),
            &theirs,
        )
        .unwrap_or((PathBuf::new(), Vec::new()));

        assert_eq!(
            read(&into.join("compose.yaml")),
            mine,
            "a reset wrote over an area the operator had declared unmanaged"
        );
        assert!(reverted.is_empty(), "{reverted:?}");
    }

    /// The command whose whole purpose is to overwrite an edit, held by the
    /// declaration whose whole purpose is to stop that.
    #[test]
    fn a_reapply_leaves_a_quality_config_declared_unmanaged_exactly_as_it_is() {
        let (into, record) = scratch("unmanaged-reapply");
        let maximum = Selection::everywhere(Preset::Maximum);
        let _ = materialise(
            Source::Embedded(&STACKLET),
            Some(&into),
            Some(&record),
            Some(&maximum),
            &[],
        );
        let recyclarr = into.join("config/recyclarr/recyclarr.yml");
        let _ = std::fs::write(&recyclarr, "# mine\n");

        let overwritten = reapply_recyclarr(
            Source::Embedded(&STACKLET),
            Some(&into),
            Some(&record),
            &maximum,
            &declared("config/recyclarr/recyclarr.yml"),
            false,
        )
        .unwrap_or_default();

        assert!(overwritten.is_none(), "{overwritten:?}");
        assert_eq!(read(&recyclarr), "# mine\n", "the config was overwritten");
    }

    #[test]
    fn reapply_leaves_an_external_stack_alone() {
        let (into, record) = scratch("reapply-external");
        let overwritten = reapply_recyclarr(
            Source::External(Path::new("/some/operator/stack")),
            Some(&into),
            Some(&record),
            &balanced(),
            &[],
            false,
        )
        .unwrap_or_default();
        assert!(
            overwritten.is_none(),
            "an external stack is the operator's, left untouched"
        );
        assert!(!into.exists(), "nothing was written for an external stack");
    }
}
