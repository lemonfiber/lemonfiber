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
pub fn is_archive(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|part| part.to_str()) else {
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
pub fn already_unpacked(occupants: &[Occupant]) -> Vec<&Occupant> {
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
mod tests {
    use std::path::{Path, PathBuf};

    use super::{already_unpacked, is_archive};
    use crate::ports::filesystem::Identity;
    use crate::ports::occupancy::Occupant;

    /// A walked file, whose identity no case here turns on.
    fn file(path: &str, bytes: u64) -> Occupant {
        Occupant {
            path: PathBuf::from(path),
            bytes,
            identity: Some(Identity { file: 1, links: 1 }),
        }
    }

    /// The paths reported as already unpacked.
    fn found(occupants: &[Occupant]) -> Vec<String> {
        already_unpacked(occupants)
            .into_iter()
            .map(|occupant| occupant.path.display().to_string())
            .collect()
    }

    #[test]
    fn the_shapes_an_archive_part_comes_in_are_recognised() {
        for name in [
            "a.rar", "a.RAR", "a.zip", "a.7z", "a.tar", "a.gz", "a.r00", "a.r99", "a.z01", "a.001",
        ] {
            assert!(is_archive(Path::new(name)), "{name} is part of an archive");
        }
    }

    #[test]
    fn what_is_not_an_archive_is_not_taken_for_one() {
        for name in ["a.mkv", "a.nfo", "a", "a.rare", "a.r0", "a.zed", "a.mp4"] {
            assert!(!is_archive(Path::new(name)), "{name} is not an archive");
        }
    }

    #[test]
    fn parts_beside_what_was_unpacked_from_them_are_the_waste() {
        // In the order they were walked in, which is the order they are given in:
        // the walk above this sorts by path, so a caller gets them sorted without
        // this sorting them a second time.
        let seen = found(&[
            file("/d/A.Release/a.r00", 500),
            file("/d/A.Release/a.rar", 500),
            file("/d/A.Release/A.Release.mkv", 1_000),
        ]);
        assert_eq!(seen, ["/d/A.Release/a.r00", "/d/A.Release/a.rar"]);
    }

    #[test]
    fn parts_with_nothing_unpacked_beside_them_are_the_only_copy() {
        // Nothing has been unpacked here yet. Removing the parts would remove the
        // whole of what was fetched, which is the opposite of reclaiming waste.
        assert!(found(&[
            file("/d/A.Release/a.rar", 500),
            file("/d/A.Release/a.r00", 500),
        ])
        .is_empty());
    }

    #[test]
    fn a_directory_of_media_alone_holds_nothing_to_reclaim() {
        assert!(found(&[
            file("/d/films/A.Film.mkv", 1_000),
            file("/d/films/A.Film.nfo", 1),
        ])
        .is_empty());
    }

    #[test]
    fn each_directory_is_judged_on_what_is_in_it_rather_than_on_its_neighbours() {
        // The unpacked file next door does not make the untouched archive beside it
        // safe to remove.
        let seen = found(&[
            file("/d/Done/a.rar", 500),
            file("/d/Done/Done.mkv", 1_000),
            file("/d/Waiting/b.rar", 500),
        ]);
        assert_eq!(seen, ["/d/Done/a.rar"]);
    }

    #[test]
    fn a_file_with_no_directory_above_it_is_still_judged() {
        assert!(found(&[file("a.rar", 500)]).is_empty());
        assert_eq!(found(&[file("a.rar", 500), file("a.mkv", 5)]), ["a.rar"]);
    }
}
