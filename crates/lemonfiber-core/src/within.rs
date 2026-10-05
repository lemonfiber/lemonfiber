//! Where a name somebody supplied lands beneath a directory.
//!
//! One rule, in one place, for every caller that turns text from outside into a
//! path underneath a directory lemonfiber chose. The app serves files by the path a
//! browser asked for, and a restore names one of the archives this machine kept;
//! both are text a request carried, and both would reach the operator's whole
//! filesystem if the text were handed to the platform as a path.
//!
//! Only ordinary names survive. A parent link is refused rather than resolved,
//! because the directory above the one a caller was given is not the one it was
//! given. Naming nothing is the empty path, which is the directory itself — what
//! that means is the caller's to say, since it is the app for one of them and
//! nothing at all for the other.

use std::path::{Path, PathBuf};

use crate::ports::filesystem::{Beneath, Confined, Fault};

/// Where `asked` lands beneath a directory, or nothing where it leads outside one.
///
/// Split on the separator a request uses rather than handed to the platform's own
/// path parser: a backslash means one thing on Windows and nothing on Linux, and a
/// rule about what may be reached must not change with the machine. A colon goes the
/// same way and for the same reason: `C:evil` names a drive rather than a file, and
/// the platform's own `join` drops the directory it was given when handed one — so a
/// name carrying either is refused rather than pushed.
#[must_use]
pub fn beneath(asked: &str) -> Option<PathBuf> {
    let mut path = PathBuf::new();
    for segment in asked.split('/') {
        match segment {
            "" | "." => {}
            ".." => return None,
            name if name.contains('\\') || name.contains(':') => return None,
            name => path.push(name),
        }
    }
    Some(path)
}

/// The one file `asked` names beneath a directory, or nothing where it names
/// anything else.
///
/// Stricter than [`beneath`] by exactly one rule: the answer is a single file in
/// the directory rather than anywhere under it. What it is for is a caller holding
/// a directory of its own files — the archives this machine kept — where a
/// subdirectory is not somewhere it ever wrote and so not somewhere to read from.
#[must_use]
pub(crate) fn one_file(asked: &str) -> Option<PathBuf> {
    let path = beneath(asked)?;
    let mut parts = path.components();
    let only = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    Some(PathBuf::from(only.as_os_str()))
}

/// Write `contents` to `path` in place, only where it is a plain file beneath `within`.
///
/// In place, because some of what lemonfiber writes this way is a file a container is
/// given on its own, and a container given one file follows that file rather than
/// whatever replaces it. Never through a link, at the file or on the way to it, because
/// the directories these files sit in are ones containers can write to: a link planted
/// there would turn lemonfiber's write into a write anywhere the operator can.
///
/// # Errors
///
/// Where the file is not a plain file beneath `within`, or the write itself fails.
pub(crate) fn write_unlinked(
    confined: &dyn Confined,
    path: &Path,
    within: &Path,
    contents: &[u8],
) -> std::io::Result<()> {
    confined
        .overwrite(path, within, contents)
        .map_err(|fault| std::io::Error::other(fault.message))
}

/// Read `path`, only where it is a plain file beneath `within`: what it holds, or
/// nothing where it is not there.
///
/// # Errors
///
/// Where something other than a plain file of `within`'s own is there, in the words a
/// write to it would be refused in.
pub(crate) fn read_unlinked(
    confined: &dyn Confined,
    path: &Path,
    within: &Path,
) -> Result<Option<String>, String> {
    match confined.read(path, within) {
        Beneath::Read(text) => Ok(Some(text)),
        Beneath::Absent => Ok(None),
        Beneath::Escaped => Err(Fault::escaped(path, within).message),
    }
}

/// The directory a file sits in, which is what a file lemonfiber writes into a
/// container's directory is held beneath.
///
/// The directory itself is safe to name: it is the one mounted into the container, and a
/// container can write what is in its mount but cannot replace the mount.
#[must_use]
pub(crate) fn directory_of(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

#[cfg(test)]
mod tests;
