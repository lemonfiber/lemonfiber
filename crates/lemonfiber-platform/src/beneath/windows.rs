//! The operations beneath a directory on Windows, which has no flag that refuses a link
//! on the way: each step is looked at first and refused where it is a link.

use std::ffi::OsStr;
use std::fs::File;
use std::io::Write as _;
use std::path::Path;

use super::{staging, steps, unmoved, unpacked, unread, unremoved, unwritten, Kept, Lies, Refusal};
use crate::fs::PERMISSION_BITS;

/// Make `path` and every directory missing above it, one at a time from `within` down,
/// each step that is already there looked at first and refused where it is not a
/// directory of its own.
///
/// # Errors
///
/// Where `path` does not lie beneath `within`, or a step is a link or a file.
pub fn make(path: &Path, within: &Path) -> Result<(), Refusal> {
    let mut here = within.to_path_buf();
    for name in steps(path, within)? {
        here.push(name);
        match std::fs::symlink_metadata(&here) {
            Ok(found) if found.is_dir() => {}
            Ok(_) => {
                return Err(Refusal::Said(format!(
                    "{} is a link or a file, and lemonfiber makes its own directory there \
                     rather than following one",
                    here.display()
                )))
            }
            Err(_) => {
                std::fs::create_dir(&here).map_err(|error| Refusal::Said(error.to_string()))?;
            }
        }
    }
    Ok(())
}

/// `path` opened to write, made where it is not there with every directory above it;
/// what was opened is for the caller to hold to `within` afterwards.
///
/// # Errors
///
/// Where a step is refused as [`make`] refuses one, or the open fails.
pub fn created(path: &Path, within: &Path, _mode: u32) -> Result<File, Refusal> {
    if let Some(parent) = path.parent() {
        make(parent, within)?;
    }
    std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open(path)
        .map_err(|error| Refusal::Said(error.to_string()))
}

/// Where `path` lies beneath `within`, each step looked at and refused where it is a link.
#[must_use]
pub fn lies(path: &Path, within: &Path) -> Lies {
    let Ok(names) = steps(path, within) else {
        return Lies::Escaped;
    };
    let mut here = within.to_path_buf();
    for name in names {
        here.push(name);
        match std::fs::symlink_metadata(&here) {
            Ok(found) if found.file_type().is_symlink() => return Lies::Escaped,
            Ok(_) => {}
            Err(_) => return Lies::Absent,
        }
    }
    Lies::Plain
}

/// Move `from` to `to`, both beneath `within`, every step of both looked at first and
/// refused where it is a link.
///
/// # Errors
///
/// Where nothing stands at `from`, something stands at `to`, or a link is on the way.
pub fn moved(from: &Path, to: &Path, within: &Path) -> Result<(), Refusal> {
    match (lies(from, within), lies(to, within)) {
        (Lies::Plain, Lies::Absent) => {}
        (Lies::Absent, _) => return Err(unmoved(from, to, "nothing stands there")),
        (_, Lies::Plain) => return Err(unmoved(from, to, "something already stands there")),
        _ => return Err(unmoved(from, to, "a link stands on the way")),
    }
    if let Some(above) = to.parent() {
        make(above, within)?;
    }
    std::fs::rename(from, to).map_err(|error| unmoved(from, to, &error.to_string()))
}

/// The directory `path`, made where it is not there, as [`make`] makes it.
///
/// # Errors
///
/// Where a step is refused as [`make`] refuses one.
pub fn made(path: &Path, within: &Path, _kept: Kept) -> Result<(), Refusal> {
    make(path, within)
}

/// `contents` written beside `path` and renamed over it, every step looked at first.
///
/// # Errors
///
/// Where a step is refused, a link stands at its name, or the write or rename fails.
pub fn replaced(path: &Path, within: &Path, contents: &[u8], kept: Kept) -> Result<(), Refusal> {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(unwritten(path, "it names no file"));
    };
    made(parent, within, kept)?;
    if lies(path, within) == Lies::Escaped {
        return Err(unwritten(path, "a link stands where it is"));
    }
    let staging = parent.join(staging(name));
    let written = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staging)
        .and_then(|mut file| file.write_all(contents).and_then(|()| file.sync_all()))
        .and_then(|()| std::fs::rename(&staging, path));
    written.map_err(|error| {
        let _ = std::fs::remove_file(&staging);
        unwritten(path, &error.to_string())
    })
}

/// What `path` holds, every step looked at first, where it is no larger than `limit`.
///
/// # Errors
///
/// Where a link is on the way, or it cannot be read or is larger than `limit`.
pub fn bytes(path: &Path, within: &Path, limit: u64) -> Result<Option<Vec<u8>>, Refusal> {
    match lies(path, within) {
        Lies::Absent => return Ok(None),
        Lies::Escaped => return Err(unread(path, "a link stands on the way")),
        Lies::Plain => {}
    }
    let held = std::fs::read(path).map_err(|error| unread(path, &error.to_string()))?;
    if held.len() as u64 > limit {
        return Err(unread(path, "it is larger than lemonfiber reads"));
    }
    Ok(Some(held))
}

/// Take away the file `path`, every step looked at first.
///
/// # Errors
///
/// Where a link is on the way, or the platform refuses the removal.
pub fn removed(path: &Path, within: &Path) -> Result<(), Refusal> {
    match lies(path, within) {
        Lies::Absent => Ok(()),
        Lies::Escaped => Err(unremoved(path, "a link stands on the way")),
        Lies::Plain => {
            std::fs::remove_file(path).map_err(|error| unremoved(path, &error.to_string()))
        }
    }
}

/// Make the directory `names` lead to beneath `root`, the making [`make`]'s, which looks
/// at each step first.
///
/// # Errors
///
/// Where a step is refused as [`make`] refuses one.
pub fn unpacked_directory(root: &Path, names: &[&OsStr], _mode: u32) -> Result<(), Refusal> {
    let path = names
        .iter()
        .fold(root.to_path_buf(), |at, name| at.join(name));
    make(&path, root)
}

/// Make the file `last` in the directory `above` leads to beneath `root`, new, holding
/// `contents`.
///
/// # Errors
///
/// Where a step is refused, something already stands at its name, or the write fails.
pub fn unpacked_file(
    root: &Path,
    above: &[&OsStr],
    last: &OsStr,
    _mode: u32,
    contents: &[u8],
) -> Result<(), Refusal> {
    let names = [above, &[last]].concat();
    let names = names.as_slice();
    unpacked_directory(root, above, PERMISSION_BITS)?;
    let path = names
        .iter()
        .fold(root.to_path_buf(), |at, name| at.join(name));
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .and_then(|mut made| made.write_all(contents))
        .map_err(|error| unpacked(root, names, &error.to_string()))
}
