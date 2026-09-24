//! Archives whose contents are already sitting beside them.
//!
//! A great deal of what a Usenet client fetches arrives as a multi-part archive
//! and is unpacked where it landed. The unpacked file is what everything
//! afterwards uses; the parts stay where they are, taking exactly as much room
//! again, and nothing in the default arrangement ever removes them. It is the
//! quietest way a disk fills, because the operator sees one film and the disk
//! holds two.
//!
//! Recognised from the walk alone, with no service asked and nothing inferred from
//! a filename beyond its extension: parts of an archive in a directory that also
//! holds something that is not an archive. A directory holding *only* archive parts
//! is left alone — nothing has been unpacked there yet, and removing the parts
//! would remove the only copy.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::ports::occupancy::Occupant;

/// The extensions this recognises as part of an archive.
///
/// The unnumbered ones, matched exactly. A numbered continuation — `.r00`, `.z01`,
/// `.001` — is recognised by its shape instead, since writing out a hundred of each
/// would be a list nobody could check.
const ARCHIVES: [&str; 5] = ["rar", "zip", "7z", "tar", "gz"];

/// Whether a path names part of an archive.
///
/// Case-insensitive, because the same release is spelled `.RAR` and `.rar` by
/// different packers and a rule that saw only one of them would find half of what
/// is there.
#[must_use]
pub(crate) fn is_archive(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(std::ffi::OsStr::to_str) else {
        return false;
    };
    let lower = extension.to_ascii_lowercase();
    if ARCHIVES.contains(&lower.as_str()) {
        return true;
    }
    // A continuation part: `r` or `z` and two digits, or three digits on their own.
    let digits = lower
        .strip_prefix('r')
        .or_else(|| lower.strip_prefix('z'))
        .unwrap_or(lower.as_str());
    let wanted = if digits.len() == lower.len() { 3 } else { 2 };
    digits.len() == wanted && digits.chars().all(|character| character.is_ascii_digit())
}

/// The archive parts in directories that also hold something unpacked.
///
/// Returned as the occupants themselves so the caller counts them the same way it
/// counts everything else — inode-aware, and against the same walk.
#[must_use]
pub(crate) fn already_unpacked(occupants: &[Occupant]) -> Vec<&Occupant> {
    let mut folders: BTreeMap<PathBuf, (Vec<&Occupant>, bool)> = BTreeMap::new();
    for occupant in occupants {
        let folder = occupant
            .path
            .parent()
            .map_or_else(PathBuf::new, Path::to_path_buf);
        let entry = folders.entry(folder).or_insert_with(|| (Vec::new(), false));
        if is_archive(&occupant.path) {
            entry.0.push(occupant);
        } else {
            entry.1 = true;
        }
    }
    folders
        .into_values()
        .filter(|(_, unpacked)| *unpacked)
        .flat_map(|(archives, _)| archives)
        .collect()
}

#[cfg(test)]
mod tests;
