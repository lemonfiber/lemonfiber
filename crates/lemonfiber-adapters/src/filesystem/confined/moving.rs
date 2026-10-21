//! Moving what stands at one path somebody else's container can write to another.
//!
//! Each end is reached from `within` one directory at a time, every step opened through
//! the handle of the one above it and never through a link, and nothing is resolved by
//! its path again after that. What is moved is looked at through its directory's handle
//! and refused unless it is a plain file or directory, so a link, a pipe or a device put
//! there is never opened; then opened, refusing a link and never waiting, and held to what
//! was looked at. The move is one rename between the two directories' handles that refuses
//! to replace anything, and what landed is held to the very file that was checked, by
//! device and number, and put back where anything else was swapped in at its old name
//! between the two.

use std::ffi::OsStr;
use std::path::Path;

use lemonfiber_ports::filesystem::Fault;

use super::steps;

/// Why a move is refused where what it found is not what it looked at.
const REPLACED: &str = "something else was put in its place";

/// Move `from` to `to`, both beneath `within`, making the directories missing above `to`.
#[cfg(unix)]
pub(in super::super) fn moved(from: &Path, to: &Path, within: &Path) -> Result<(), Fault> {
    moving(from, to, within, &|| ())
}

/// The same, with `meanwhile` run between checking what is moved and moving it: nothing
/// in a run, and in a test whatever another writer could do in that moment.
#[cfg(unix)]
fn moving(from: &Path, to: &Path, within: &Path, meanwhile: &dyn Fn()) -> Result<(), Fault> {
    use rustix::fs::{fstat, openat, renameat_with, statat, AtFlags, Mode, RenameFlags, CWD};
    let unmoved = |error: rustix::io::Errno| refused(from, to, &error.to_string());
    let (from_name, from_above) = ends(from, within)?;
    let (to_name, to_above) = ends(to, within)?;
    let root = openat(CWD, within, opening(), Mode::empty()).map_err(unmoved)?;
    let source = walked(&root, &from_above, false).map_err(unmoved)?;
    let looked = statat(&source, from_name, AtFlags::SYMLINK_NOFOLLOW).map_err(unmoved)?;
    if let Some(why) = unplain(&looked) {
        return Err(refused(from, to, why));
    }
    let checked = openat(&source, from_name, held(), Mode::empty())
        .and_then(|opened| fstat(&opened))
        .map_err(unmoved)?;
    (unplain(&checked).is_none() && same(&looked, &checked))
        .then_some(())
        .ok_or(refused(from, to, REPLACED))?;
    let target = walked(&root, &to_above, true).map_err(unmoved)?;
    meanwhile();
    renameat_with(&source, from_name, &target, to_name, RenameFlags::NOREPLACE).map_err(
        |error| match error {
            rustix::io::Errno::EXIST | rustix::io::Errno::NOTEMPTY => {
                refused(from, to, "something already stands there")
            }
            other => unmoved(other),
        },
    )?;
    let landed = statat(&target, to_name, AtFlags::SYMLINK_NOFOLLOW);
    if !landed.is_ok_and(|landed| same(&landed, &checked)) {
        let _ = renameat_with(&target, to_name, &source, from_name, RenameFlags::NOREPLACE);
        return Err(refused(from, to, REPLACED));
    }
    Ok(())
}

/// Move `from` to `to`, both beneath `within`. Windows has no flag that refuses a link on
/// the way, so every step of both is looked at first and refused where it is a link.
#[cfg(windows)]
pub(in super::super) fn moved(from: &Path, to: &Path, within: &Path) -> Result<(), Fault> {
    use lemonfiber_ports::filesystem::Lying;
    match (super::lies(from, within), super::lies(to, within)) {
        (Lying::Plain, Lying::Absent) => {}
        (Lying::Absent, _) => return Err(refused(from, to, "nothing stands there")),
        (_, Lying::Plain) => return Err(refused(from, to, "something already stands there")),
        _ => return Err(refused(from, to, "a link stands on the way")),
    }
    if let Some(above) = to.parent() {
        super::make(above, within)?;
    }
    std::fs::rename(from, to).map_err(|error| refused(from, to, &error.to_string()))
}

/// The last name of `path` beneath `within` and the names of the directories above it.
fn ends<'a>(path: &'a Path, within: &Path) -> Result<(&'a OsStr, Vec<&'a OsStr>), Fault> {
    let mut names = steps(path, within)?;
    let last = names.pop().ok_or_else(|| {
        Fault::new(format!(
            "{} names {} itself, and lemonfiber moves only what lies beneath it",
            path.display(),
            within.display()
        ))
    })?;
    Ok((last, names))
}

/// How every directory on the way is opened: as a directory, never waiting on a pipe put
/// where one was, and never left to a child.
#[cfg(unix)]
fn opening() -> rustix::fs::OFlags {
    use rustix::fs::OFlags;
    OFlags::DIRECTORY | OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC
}

/// The directory `names` lead to from `root`, each step opened through the one above it
/// and never through a link, each made first where `making` says so.
#[cfg(unix)]
fn walked(
    root: &rustix::fd::OwnedFd,
    names: &[&OsStr],
    making: bool,
) -> rustix::io::Result<rustix::fd::OwnedFd> {
    use rustix::fs::{mkdirat, openat, Mode, OFlags};
    let mut here = openat(root, ".", opening(), Mode::empty())?;
    for name in names {
        if making {
            match mkdirat(&here, *name, Mode::RWXU | Mode::RWXG | Mode::RWXO) {
                Ok(()) | Err(rustix::io::Errno::EXIST) => {}
                Err(error) => return Err(error),
            }
        }
        here = openat(&here, *name, opening() | OFlags::NOFOLLOW, Mode::empty())?;
    }
    Ok(here)
}

/// How what is moved is opened: to be looked at and nothing more, never through a link at
/// its own name, and never waiting on a pipe put where it was.
#[cfg(unix)]
fn held() -> rustix::fs::OFlags {
    use rustix::fs::OFlags;
    let opening = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    let opening = opening | OFlags::PATH;
    opening
}

/// Why `found` is not moved, where it is not a plain file or a directory.
#[cfg(unix)]
fn unplain(found: &rustix::fs::Stat) -> Option<&'static str> {
    use rustix::fs::FileType;
    match FileType::from_raw_mode(found.st_mode) {
        FileType::RegularFile | FileType::Directory => None,
        FileType::Symlink => Some("it is a link"),
        _ => Some("it is neither a file nor a directory"),
    }
}

/// Whether two readings are of one file: the same device and the same number on it.
#[cfg(unix)]
fn same(one: &rustix::fs::Stat, other: &rustix::fs::Stat) -> bool {
    one.st_dev == other.st_dev && one.st_ino == other.st_ino
}

/// The refusal of moving `from` to `to`, saying `why`.
fn refused(from: &Path, to: &Path, why: &str) -> Fault {
    Fault::new(format!(
        "{} was not moved to {}: {why}, and lemonfiber never moves through a link or over \
         something already there",
        from.display(),
        to.display()
    ))
}

#[cfg(test)]
mod tests;
