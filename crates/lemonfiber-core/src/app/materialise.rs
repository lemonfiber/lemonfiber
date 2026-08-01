//! Writing the embedded stack to disk without clobbering an operator's edits.
//!
//! The stack lemonfiber ships is written out so Compose can read it, on every
//! invocation rather than cached, so an upgrade never leaves a stale copy behind.
//! Those files are the operator's to edit, though, so a blind re-write would lose a
//! change they made by hand on the next run. This writes each file only where it is
//! safe to — absent, or still holding what lemonfiber last wrote — and leaves a file
//! the operator has changed exactly as it is, reporting it with a diff rather than
//! overwriting it. An external stack is theirs entirely and is left untouched.

use std::path::{Path, PathBuf};

use crate::config::store;
use crate::materialised::{checksum, decide, diff, Decision, Materialised};
use crate::model::StackEdit;
use crate::stack::{Failure, Source};

/// Write the stack under `into`, preserving any file the operator has edited, and
/// return where it lives and which files were left as they set them.
///
/// An external stack is already on disk and is returned as it is. An embedded stack
/// is written file by file: each is compared, by content, against what lemonfiber
/// last wrote (read from `record_path`) so an operator's edit is told from a version
/// not yet upgraded — the edit is preserved and reported, the rest written. The
/// record of what was written is updated for the next run.
///
/// # Errors
///
/// Returns [`Failure`] when there is nowhere to write an embedded stack to, or when
/// a file cannot be written.
pub(super) fn materialise(
    source: Source,
    into: Option<&Path>,
    record_path: Option<&Path>,
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
        let target = into.join(&relative);
        let desired = checksum(content);
        let on_disk = std::fs::read(&target).ok();
        let actual = on_disk.as_deref().map(checksum);
        match decide(record.checksum(&key), actual, desired) {
            Decision::Write => {
                write(&target, content)?;
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
                        &String::from_utf8_lossy(content),
                    ),
                });
            }
        }
    }
    save(record_path, &record);
    Ok((into.to_path_buf(), edits))
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

    use super::materialise;
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

    #[test]
    fn every_file_is_written_and_recorded_then_left_on_a_second_run() {
        let (into, record) = scratch("write-and-leave");
        let source = Source::Embedded(&STACKLET);

        let first = materialise(source, Some(&into), Some(&record));
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
        let (_, again) =
            materialise(source, Some(&into), Some(&record)).unwrap_or((PathBuf::new(), Vec::new()));
        assert!(again.is_empty(), "an unchanged file is left, not reported");
    }

    #[test]
    fn an_edited_file_is_preserved_and_reported_with_a_diff() {
        let (into, record) = scratch("preserve-edit");
        let source = Source::Embedded(&STACKLET);
        let _ = materialise(source, Some(&into), Some(&record));

        // The operator edits a materialised file by hand.
        let edited = "services:\n  sonarr:\n    image: my-own-sonarr\n";
        let _ = std::fs::write(into.join("compose.yaml"), edited);

        let (_, edits) =
            materialise(source, Some(&into), Some(&record)).unwrap_or((PathBuf::new(), Vec::new()));
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
        let (path, edits) = materialise(Source::External(external), Some(&into), Some(&record))
            .unwrap_or((PathBuf::new(), vec![]));
        assert_eq!(path, external, "an external stack is used where it lives");
        assert!(edits.is_empty());
        assert!(!into.exists(), "nothing was written for an external stack");
    }

    #[test]
    fn an_embedded_stack_with_nowhere_to_write_is_refused() {
        let refusal = materialise(Source::Embedded(&STACKLET), None, None);
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
        let failure = materialise(Source::Embedded(&STACKLET), Some(&into), Some(&record));
        assert!(matches!(failure, Err(Failure::NotWritten { .. })));
    }

    #[test]
    fn without_a_record_path_the_stack_is_still_written() {
        let (into, _) = scratch("no-record");
        let (_, edits) = materialise(Source::Embedded(&STACKLET), Some(&into), None)
            .unwrap_or((PathBuf::new(), vec![]));
        assert!(edits.is_empty());
        assert!(read(&into.join("stack.toml")).contains("stacklet"));
    }
}
