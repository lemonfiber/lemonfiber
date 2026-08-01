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
) -> Result<(PathBuf, Vec<StackEdit>), Failure> {
    let files = source.files();
    if files.is_empty() {
        // External, or nothing to write: left exactly as it is.
        return source.materialise(into).map(|path| (path, Vec::new()));
    }
    let Some(into) = into else {
        return Err(Failure::NowhereToWrite);
    };

    let mut record = load(record_path);
    let mut edits = Vec::new();
    for (relative, content) in files {
        let key = relative.to_string_lossy();
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
            Decision::Write => {
                write(&target, &content)?;
                record.record(&key, desired);
            }
            // Already what lemonfiber would write: left, and recorded so the next run
            // reads it as lemonfiber's own rather than the operator's.
            Decision::Fresh => record.record(&key, desired),
            // The operator's edit: left as it is, its record kept at what lemonfiber
            // last wrote so it goes on being recognised, and shown with a diff.
            Decision::Preserve => {
                let yours = on_disk.unwrap_or_default();
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
    save(record_path, &record);
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
/// Returns whether it overwrote a customised config, as opposed to one already in
/// lemonfiber's own hand. A rehearsal reports what it would do and writes nothing. An
/// external stack, which lemonfiber does not materialise, is left untouched.
///
/// # Errors
///
/// Returns [`Failure`] when there is nowhere to write, or the config cannot be written.
pub(super) fn reapply_recyclarr(
    source: Source,
    into: Option<&Path>,
    record_path: Option<&Path>,
    selection: &Selection,
    rehearse: bool,
) -> Result<bool, Failure> {
    let Some(shipped) = shipped_recyclarr(source) else {
        // External, or a stack with no Recyclarr config: nothing lemonfiber manages.
        // This is also the guard that keeps an external stack safe — `into` is the
        // built-in stack directory, not where an external stack lives, so writing
        // there would be wrong. An external source has no embedded files, so it
        // returns here before touching `into`; that invariant is load-bearing.
        return Ok(false);
    };
    let Some(into) = into else {
        return Err(Failure::NowhereToWrite);
    };
    let was_customised = recyclarr_customised(Some(into), record_path);
    if !rehearse {
        let desired = crate::recyclarr::rewrite(&String::from_utf8_lossy(shipped), selection);
        let target = into.join(RECYCLARR_CONFIG);
        write(&target, desired.as_bytes())?;
        let mut record = load(record_path);
        record.record(RECYCLARR_CONFIG, checksum(desired.as_bytes()));
        save(record_path, &record);
    }
    Ok(was_customised)
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
    record_path
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
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

    use super::{materialise, reapply_recyclarr, recyclarr_customised};
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

        let first = materialise(source, Some(&into), Some(&record), Some(&balanced()));
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
        let (_, again) = materialise(source, Some(&into), Some(&record), Some(&balanced()))
            .unwrap_or((PathBuf::new(), Vec::new()));
        assert!(again.is_empty(), "an unchanged file is left, not reported");
    }

    #[test]
    fn an_edited_file_is_preserved_and_reported_with_a_diff() {
        let (into, record) = scratch("preserve-edit");
        let source = Source::Embedded(&STACKLET);
        let _ = materialise(source, Some(&into), Some(&record), Some(&balanced()));

        // The operator edits a materialised file by hand.
        let edited = "services:\n  sonarr:\n    image: my-own-sonarr\n";
        let _ = std::fs::write(into.join("compose.yaml"), edited);

        let (_, edits) = materialise(source, Some(&into), Some(&record), Some(&balanced()))
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
    fn an_external_stack_is_returned_and_nothing_is_written() {
        let (into, record) = scratch("external");
        let external = Path::new("/some/operator/stack");
        let (path, edits) = materialise(
            Source::External(external),
            Some(&into),
            Some(&record),
            Some(&balanced()),
        )
        .unwrap_or((PathBuf::new(), vec![]));
        assert_eq!(path, external, "an external stack is used where it lives");
        assert!(edits.is_empty());
        assert!(!into.exists(), "nothing was written for an external stack");
    }

    #[test]
    fn an_embedded_stack_with_nowhere_to_write_is_refused() {
        let refusal = materialise(Source::Embedded(&STACKLET), None, None, Some(&balanced()));
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
        )
        .unwrap_or((PathBuf::new(), vec![]));
        // Written, not reported as an edit: this is lemonfiber's own choice landing.
        assert!(edits.is_empty());
        let written = read(&into.join("config/recyclarr/recyclarr.yml"));
        // The 4K templates are in force, and the Balanced ones the fixture shipped
        // with are gone.
        assert!(written.contains("sonarr-v4-quality-profile-web-2160p"));
        assert!(written.contains("radarr-quality-profile-uhd-bluray-web"));
        assert!(!written.contains("sonarr-v4-quality-profile-web-1080p"));
    }

    #[test]
    fn the_default_choice_leaves_the_shipped_recyclarr_config_untouched() {
        let (into, record) = scratch("recyclarr-default");
        let (_, edits) = materialise(
            Source::Embedded(&STACKLET),
            Some(&into),
            Some(&record),
            Some(&balanced()),
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
        );
        let recyclarr = into.join("config/recyclarr/recyclarr.yml");
        assert!(read(&recyclarr).contains("sonarr-v4-quality-profile-web-2160p"));

        // A later command carrying no choice leaves the applied preset exactly as it
        // is — not written back to the shipped default.
        let _ = materialise(
            Source::Embedded(&STACKLET),
            Some(&into),
            Some(&record),
            None,
        );
        assert!(
            read(&recyclarr).contains("sonarr-v4-quality-profile-web-2160p"),
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
        );
        let recyclarr = into.join("config/recyclarr/recyclarr.yml");
        // The operator hand-edits it.
        let _ = std::fs::write(&recyclarr, "# mine\n");
        assert!(recyclarr_customised(Some(&into), Some(&record)));

        // Reapply re-asserts the recorded preset over the edit.
        let overwrote = reapply_recyclarr(
            Source::Embedded(&STACKLET),
            Some(&into),
            Some(&record),
            &maximum,
            false,
        )
        .unwrap_or(false);
        assert!(overwrote, "an edit was overwritten");
        assert!(read(&recyclarr).contains("sonarr-v4-quality-profile-web-2160p"));
        // Recorded as lemonfiber's own again: no longer customised.
        assert!(!recyclarr_customised(Some(&into), Some(&record)));
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
        );
        let recyclarr = into.join("config/recyclarr/recyclarr.yml");
        let _ = std::fs::write(&recyclarr, "# mine\n");

        let overwrote = reapply_recyclarr(
            Source::Embedded(&STACKLET),
            Some(&into),
            Some(&record),
            &maximum,
            true,
        )
        .unwrap_or(false);
        assert!(
            overwrote,
            "the rehearsal reports it would overwrite an edit"
        );
        // The edit is still on disk: a rehearsal changed nothing.
        assert_eq!(read(&recyclarr), "# mine\n");
    }

    #[test]
    fn reapply_leaves_an_external_stack_alone() {
        let (into, record) = scratch("reapply-external");
        let overwrote = reapply_recyclarr(
            Source::External(Path::new("/some/operator/stack")),
            Some(&into),
            Some(&record),
            &balanced(),
            false,
        )
        .unwrap_or(true);
        assert!(
            !overwrote,
            "an external stack is the operator's, left untouched"
        );
        assert!(!into.exists(), "nothing was written for an external stack");
    }
}
