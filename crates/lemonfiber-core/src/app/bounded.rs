//! Writing a region into one of the stack's own files, and taking it back out.
//!
//! The write and its undo are the two halves of [`crate::journal::Kind::Region`], and
//! both halves do one more thing than edit the file: they keep the record of what
//! lemonfiber materialised true. That record is how the pass that writes the stack's
//! files tells an operator's edit from lemonfiber's own. A region written without
//! re-recording the file would read to that pass as the operator's edit, and the
//! file would be preserved as theirs from then on, never updated again. So the
//! checksum moves with the region, inside the same write, and only where the record
//! was already holding the file as lemonfiber last wrote it. A file the operator had
//! edited before the region went in is still theirs afterwards, and still reads that
//! way.
//!
//! Where the file is kept is the caller's business. The journal entry is written
//! before the region is, by the caller, for the reason every write is journalled
//! first: a run that dies between the two leaves a record of a region that may not be
//! there, and taking out a region that is not there is nothing.

use std::path::Path;

use crate::materialised::{checksum, Materialised};

/// Where the record of what lemonfiber materialised is kept, beside the settings file.
///
/// The same answer the lifecycle commands derive, so the record a region re-records
/// is the one the next pass over the stack reads.
pub(crate) fn record_beside(env_file: &Path) -> std::path::PathBuf {
    env_file.with_file_name("materialised.json")
}

/// Write `owner`'s region holding `body` into the file at `path`.
///
/// The file has to be there already: a region is written into a file the stack has,
/// never into one this brings into being.
///
/// # Errors
///
/// Where the file cannot be read or written, in the operating system's own words.
pub(crate) fn put(
    path: &Path,
    key: &str,
    owner: &str,
    body: &str,
    record: Option<&Path>,
) -> Result<(), String> {
    let before = std::fs::read_to_string(path).map_err(|why| why.to_string())?;
    let after = crate::region::put(&before, owner, body);
    rewritten(path, key, &before, &after, record)
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
    path: &Path,
    key: &str,
    owner: &str,
    written: u32,
    record: Option<&Path>,
) -> Result<Withdrawn, String> {
    let before = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => return Ok(Withdrawn::Done),
        Err(why) => return Err(why.to_string()),
    };
    match crate::region::within(&before, owner) {
        Some(body) if checksum(body.as_bytes()) == written => {
            let after = crate::region::without(&before, owner).unwrap_or_default();
            rewritten(path, key, &before, &after, record).map(|()| Withdrawn::Done)
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
    path: &Path,
    key: &str,
    before: &str,
    after: &str,
    record: Option<&Path>,
) -> Result<(), String> {
    // Written in place, as the file it already is: its mode stays what the stack gave it,
    // because the container reading it may not be its owner, and it stays the same file,
    // because a container given one file mounts that file and not whatever replaces it.
    std::fs::write(path, after).map_err(|why| why.to_string())?;
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
