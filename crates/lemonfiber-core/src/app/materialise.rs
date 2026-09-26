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
pub(crate) fn materialise(
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
pub(crate) fn reset_stack(
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
pub(crate) fn would_materialise(
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
pub(crate) fn pending_reverts(
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
        let on_disk = std::fs::read(&target).ok();
        let content = carrying_regions(content, on_disk.as_deref());
        let desired = checksum(&content);
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

/// What lemonfiber intends for a stack file, with every region written into it since
/// carried over from the copy on disk.
///
/// A region is lemonfiber's own writing — a plugin's route through the proxy, its entry
/// on the dashboard — so what lemonfiber intends for the file is the shipped content
/// and the regions both. Intending the shipped content alone would read the file as
/// out of date and write the shipped copy over the regions, taking a plugin off the
/// proxy while it is still installed. The same on every pass: a reset puts back what
/// the operator changed and keeps what lemonfiber wrote, and a preview says so.
fn carrying_regions<'a>(content: Cow<'a, [u8]>, on_disk: Option<&[u8]>) -> Cow<'a, [u8]> {
    let carried = match (
        on_disk.map(String::from_utf8_lossy),
        std::str::from_utf8(&content),
    ) {
        (Some(disk), Ok(desired)) => {
            Some(crate::region::carried(&disk, desired)).filter(|carried| carried != desired)
        }
        _ => None,
    };
    carried.map_or(content, |carried| Cow::Owned(carried.into_bytes()))
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
pub(crate) fn recyclarr_customised(into: Option<&Path>, record_path: Option<&Path>) -> bool {
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
pub(crate) fn reapply_recyclarr(
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
    crate::within::write_unlinked(target, content)
}

#[cfg(test)]
mod tests;
