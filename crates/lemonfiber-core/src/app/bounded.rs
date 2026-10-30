//! Taking a region back out of one of the stack's own files.
//!
//! It keeps the record of what lemonfiber materialised true. That record is how the pass
//! that writes the stack's files tells an operator's edit from lemonfiber's own, so a
//! write that did not re-record the file would read to that pass as the operator's edit,
//! and the file would be preserved as theirs from then on, never updated again. So the
//! checksum moves with the text, inside the same write, and only where the record was
//! already holding the file as lemonfiber last wrote it. A file the operator had edited
//! before is still theirs afterwards, and still reads that way.
//!
//! It is the undo of [`crate::journal::Kind::Region`], which an earlier release journalled
//! when it wrote a plugin's region into one of the stack's files.

use std::path::Path;

use crate::materialised::{checksum, Materialised};
use crate::ports::filesystem::Confined;
use crate::within::{directory_of, read_unlinked, write_unlinked};

/// Where the record of what lemonfiber materialised is kept, beside the settings file.
///
/// The same answer the lifecycle commands derive, so the record a region re-records
/// is the one the next pass over the stack reads.
pub(crate) fn record_beside(env_file: &Path) -> std::path::PathBuf {
    env_file.with_file_name(crate::config::paths::MATERIALISED)
}

/// What taking a region back out came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Withdrawn {
    /// It was taken out, or there was nothing left of it to take.
    Done,
    /// It is not what was written any more — edited, or with its markers edited — so
    /// it was left exactly where it is.
    TheirsNow,
}

/// Take `owner`'s region back out of the file at `path`, where it is still exactly
/// the region that was written.
///
/// A file that is gone has no region left in it, which is nothing to do. The
/// judgement before a reversal already refuses a region somebody has edited; this
/// asks again because the file is read here and not there, and a region edited in
/// between is still not lemonfiber's to take.
///
/// # Errors
///
/// Where the file is there and cannot be read or written.
pub(crate) fn withdraw(
    confined: &dyn Confined,
    path: &Path,
    key: &str,
    owner: &str,
    written: u32,
    record: Option<&Path>,
) -> Result<Withdrawn, String> {
    let Some(before) = read_unlinked(confined, path, directory_of(path))? else {
        return Ok(Withdrawn::Done);
    };
    match crate::region::within(&before, owner) {
        Some(body) if checksum(body.as_bytes()) == written => {
            let after = crate::region::without(&before, owner).unwrap_or_default();
            rewritten(confined, path, (key, &before), &after, record).map(|()| Withdrawn::Done)
        }
        _ => Ok(Withdrawn::TheirsNow),
    }
}

/// The checksum a region's body is journalled under.
#[must_use]
pub(crate) fn written(body: &str) -> u32 {
    checksum(body.as_bytes())
}

/// Write the file's new text, and carry the record of what was materialised with it
/// where the record was holding the file as it stood.
fn rewritten(
    confined: &dyn Confined,
    path: &Path,
    (key, before): (&str, &str),
    after: &str,
    record: Option<&Path>,
) -> Result<(), String> {
    // Written in place, as the file it already is: its mode stays what the stack gave it,
    // because the container reading it may not be its owner, and it stays the same file,
    // because a container given one file mounts that file and not whatever replaces it.
    //
    // And never through a link, because the directory it is in is one a container can
    // write to.
    write_unlinked(confined, path, directory_of(path), after.as_bytes())
        .map_err(|why| why.to_string())?;
    let mut kept: Materialised = super::record::kept(record);
    if kept.checksum(key) == Some(checksum(before.as_bytes())) {
        kept.record(key, checksum(after.as_bytes()));
        // Best effort, as every write of this record is: a record that could not be
        // kept costs the next pass preserving a file it could have left as it was,
        // which is the safe direction.
        let _ = super::record::keep(record, &kept);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
