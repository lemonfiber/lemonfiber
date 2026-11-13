//! The operations beneath a directory on Unix: every step opened through the handle of
//! the one above it.

use std::ffi::OsStr;
use std::fs::File;
use std::io::{Read as _, Write as _};
use std::path::Path;

use rustix::fd::OwnedFd;
use rustix::fs::{
    fstat, fsync, mkdirat, openat, renameat, renameat_with, statat, unlinkat, AtFlags, FileType,
    Mode, OFlags, RawMode, RenameFlags, Stat, CWD,
};
use rustix::io::Errno;

use super::{staging, steps, unmoved, unpacked, unread, unremoved, unwritten, Kept, Lies, Refusal};
use crate::fs::{PERMISSION_BITS, SHARED_DIRECTORY};

/// Why a move is refused where what it found is not what it looked at.
const REPLACED: &str = "something else was put in its place";

/// Make `path` and every directory missing above it, one at a time from `within` down,
/// each made and then opened through the handle of the one above it and never through
/// a link, so a link put on the way between two steps is refused rather than followed.
///
/// # Errors
///
/// Where `path` does not lie beneath `within`, or a step is refused.
pub fn make(path: &Path, within: &Path) -> Result<(), Refusal> {
    opened_made(path, within).map(drop)
}

/// `path` opened to write, made new with `mode` where it is not there, through the
/// handle of the directory it is in, every step to which was reached through no link,
/// and refusing a link at its own name.
///
/// # Errors
///
/// [`Refusal::Escaped`] where a link stands at its name or it names no file; the
/// platform's words where a step or the open is refused otherwise.
pub fn created(path: &Path, within: &Path, mode: u32) -> Result<File, Refusal> {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(Refusal::Escaped);
    };
    let here = opened_made(parent, within)?;
    let flags =
        OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
    openat(&here, name, flags, permitted(mode))
        .map(File::from)
        .map_err(|error| match error {
            Errno::LOOP => Refusal::Escaped,
            error => Refusal::Said(error.to_string()),
        })
}

/// The directory `path`, made with every directory missing above it as [`make`] makes
/// them, held open by the handle the walk ended on.
fn opened_made(path: &Path, within: &Path) -> Result<OwnedFd, Refusal> {
    let opening = OFlags::DIRECTORY | OFlags::RDONLY | OFlags::CLOEXEC;
    let unmade = |error: Errno| {
        Refusal::Said(format!(
            "{} was not made beneath {}: {error}",
            path.display(),
            within.display()
        ))
    };
    let mut here = openat(CWD, within, opening, Mode::empty()).map_err(unmade)?;
    for name in steps(path, within)? {
        match mkdirat(&here, name, permitted(SHARED_DIRECTORY)) {
            Ok(()) | Err(Errno::EXIST) => {}
            Err(error) => return Err(unmade(error)),
        }
        here = openat(&here, name, opening | OFlags::NOFOLLOW, Mode::empty()).map_err(unmade)?;
    }
    Ok(here)
}

/// Where `path` lies beneath `within`: each directory on the way opened through the handle
/// of the one above it, never through a link and never waiting on what it opens, and the
/// last step only looked at, so a pipe or a device put there is never opened.
#[must_use]
pub fn lies(path: &Path, within: &Path) -> Lies {
    let Ok(mut names) = steps(path, within) else {
        return Lies::Escaped;
    };
    let opening = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK | OFlags::DIRECTORY;
    let Ok(mut here) = openat(CWD, within, opening, Mode::empty()) else {
        return Lies::Absent;
    };
    let Some(last) = names.pop() else {
        return Lies::Plain;
    };
    for name in names {
        match openat(&here, name, opening | OFlags::NOFOLLOW, Mode::empty()) {
            Ok(opened) => here = opened,
            Err(Errno::NOENT) => return Lies::Absent,
            Err(_) => return Lies::Escaped,
        }
    }
    match statat(&here, last, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(found) if FileType::from_raw_mode(found.st_mode) == FileType::Symlink => Lies::Escaped,
        Ok(_) => Lies::Plain,
        Err(Errno::NOENT) => Lies::Absent,
        Err(_) => Lies::Escaped,
    }
}

/// Move `from` to `to`, both beneath `within`, making the directories missing above `to`.
///
/// Each end is reached from `within` one directory at a time, and nothing is resolved by
/// its path again after that. What is moved is looked at through its directory's handle
/// and refused unless it is a plain file or directory, then opened, refusing a link and
/// never waiting, and held to what was looked at. The move is one rename between the two
/// directories' handles that refuses to replace anything, and what landed is held to the
/// very file that was checked and put back where anything else was swapped in at its old
/// name between the two.
///
/// # Errors
///
/// Where either end does not lie beneath `within`, a link or something already there is
/// met, or what is moved is not a plain file or directory.
pub fn moved(from: &Path, to: &Path, within: &Path) -> Result<(), Refusal> {
    moving(from, to, within, &|| ())
}

/// The same, with `meanwhile` run between checking what is moved and moving it: nothing
/// in a run, and in a test whatever another writer could do in that moment.
fn moving(from: &Path, to: &Path, within: &Path, meanwhile: &dyn Fn()) -> Result<(), Refusal> {
    let refused = |error: Errno| unmoved(from, to, &error.to_string());
    let (from_name, from_above) = ends(from, within)?;
    let (to_name, to_above) = ends(to, within)?;
    let root = openat(CWD, within, opening(), Mode::empty()).map_err(refused)?;
    let source = walked_from(&root, &from_above, false).map_err(refused)?;
    let looked = statat(&source, from_name, AtFlags::SYMLINK_NOFOLLOW).map_err(refused)?;
    if let Some(why) = unplain(&looked) {
        return Err(unmoved(from, to, why));
    }
    let checked = openat(&source, from_name, held(), Mode::empty())
        .and_then(|opened| fstat(&opened))
        .map_err(refused)?;
    (unplain(&checked).is_none() && same(&looked, &checked))
        .then_some(())
        .ok_or(unmoved(from, to, REPLACED))?;
    let target = walked_from(&root, &to_above, true).map_err(refused)?;
    meanwhile();
    renameat_with(&source, from_name, &target, to_name, RenameFlags::NOREPLACE).map_err(
        |error| match error {
            Errno::EXIST | Errno::NOTEMPTY => unmoved(from, to, "something already stands there"),
            other => refused(other),
        },
    )?;
    let landed = statat(&target, to_name, AtFlags::SYMLINK_NOFOLLOW);
    if !landed.is_ok_and(|landed| same(&landed, &checked)) {
        let _ = renameat_with(&target, to_name, &source, from_name, RenameFlags::NOREPLACE);
        return Err(unmoved(from, to, REPLACED));
    }
    Ok(())
}

/// The last name of `path` beneath `within` and the names of the directories above it.
fn ends<'a>(path: &'a Path, within: &Path) -> Result<(&'a OsStr, Vec<&'a OsStr>), Refusal> {
    let mut names = steps(path, within)?;
    let last = names.pop().ok_or_else(|| {
        Refusal::Said(format!(
            "{} names {} itself, and lemonfiber moves only what lies beneath it",
            path.display(),
            within.display()
        ))
    })?;
    Ok((last, names))
}

/// How every directory on the way of a move is opened: as a directory, never waiting on
/// a pipe put where one was, and never left to a child.
fn opening() -> OFlags {
    OFlags::DIRECTORY | OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC
}

/// The directory `names` lead to from `root`, each step opened through the one above it
/// and never through a link, each made first where `making` says so.
fn walked_from(root: &OwnedFd, names: &[&OsStr], making: bool) -> rustix::io::Result<OwnedFd> {
    let mut here = openat(root, ".", opening(), Mode::empty())?;
    for name in names {
        if making {
            match mkdirat(&here, *name, permitted(SHARED_DIRECTORY)) {
                Ok(()) | Err(Errno::EXIST) => {}
                Err(error) => return Err(error),
            }
        }
        here = openat(&here, *name, opening() | OFlags::NOFOLLOW, Mode::empty())?;
    }
    Ok(here)
}

/// How what is moved is opened: to be looked at and nothing more, never through a link at
/// its own name, and never waiting on a pipe put where it was.
fn held() -> OFlags {
    let opening = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    let opening = opening | OFlags::PATH;
    opening
}

/// Why `found` is not moved, where it is not a plain file or a directory.
fn unplain(found: &Stat) -> Option<&'static str> {
    match FileType::from_raw_mode(found.st_mode) {
        FileType::RegularFile | FileType::Directory => None,
        FileType::Symlink => Some("it is a link"),
        _ => Some("it is neither a file nor a directory"),
    }
}

/// Whether two readings are of one file: the same device and the same number on it.
fn same(one: &Stat, other: &Stat) -> bool {
    one.st_dev == other.st_dev && one.st_ino == other.st_ino
}

/// The directory `path`, made where it is not there, with every directory above it as
/// `kept` says.
///
/// # Errors
///
/// Where a step is a link, is not a directory, or is somebody else's where `kept` asks
/// for the writer's own.
pub fn made(path: &Path, within: &Path, kept: Kept) -> Result<(), Refusal> {
    walked(path, within, kept, true).map(drop)
}

/// `contents` written beside `path` and renamed over it, so a link planted at its name is
/// replaced rather than followed and a reader never sees half of it.
///
/// The file is written new under a staging name, so nothing already standing there is
/// written through, flushed, and renamed over the old one.
///
/// # Errors
///
/// Where a step is refused as [`made`] refuses one, or the write or the rename fails.
pub fn replaced(path: &Path, within: &Path, contents: &[u8], kept: Kept) -> Result<(), Refusal> {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(unwritten(path, "it names no file"));
    };
    let here = walked(parent, within, kept, true)?
        .ok_or(unwritten(path, "the directory it is in could not be made"))?;
    let staging = staging(name);
    let created = OFlags::RDWR
        | OFlags::CREATE
        | OFlags::EXCL
        | OFlags::NOFOLLOW
        | OFlags::NONBLOCK
        | OFlags::CLOEXEC;
    let _ = unlinkat(&here, &staging, AtFlags::empty());
    let file = openat(&here, &staging, created, permitted(kept.file))
        .map(File::from)
        .map_err(|error| unwritten(path, &error.to_string()))?;
    let written = (&file)
        .write_all(contents)
        .and_then(|()| file.sync_all())
        .map_err(worded)
        .and_then(|()| renameat(&here, &staging, &here, name).map_err(worded));
    if let Err(why) = written {
        let _ = unlinkat(&here, &staging, AtFlags::empty());
        return Err(unwritten(path, &why));
    }
    let _ = fsync(&here);
    Ok(())
}

/// What `path` holds, reached through no link, where it is a plain file no larger than
/// `limit`; nothing where it is not there.
///
/// # Errors
///
/// Where a step is a link, it is not a plain file, or it is larger than `limit`.
pub fn bytes(path: &Path, within: &Path, limit: u64) -> Result<Option<Vec<u8>>, Refusal> {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(unread(path, "it names no file"));
    };
    let Some(here) = walked(parent, within, Kept::SHARED, false)? else {
        return Ok(None);
    };
    let opening = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
    let file = match openat(&here, name, opening, Mode::empty()) {
        Ok(file) => file,
        Err(Errno::NOENT) => return Ok(None),
        Err(error) => return Err(unread(path, &linked(error))),
    };
    let plain = fstat(&file)
        .is_ok_and(|stat| FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile);
    if !plain {
        return Err(unread(path, "it is not a plain file"));
    }
    let mut held = Vec::new();
    let read = File::from(file).take(limit + 1).read_to_end(&mut held).ok();
    match read {
        Some(length) if length as u64 <= limit => Ok(Some(held)),
        _ => Err(unread(
            path,
            "it could not be read whole, or is larger than lemonfiber reads",
        )),
    }
}

/// Take away the file `path`, through the handle of the directory it is in.
///
/// # Errors
///
/// Where a step is a link, or the platform refuses the removal.
pub fn removed(path: &Path, within: &Path) -> Result<(), Refusal> {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(unremoved(path, "it names no file"));
    };
    let Some(here) = walked(parent, within, Kept::SHARED, false)? else {
        return Ok(());
    };
    match unlinkat(&here, name, AtFlags::empty()) {
        Ok(()) | Err(Errno::NOENT) => Ok(()),
        Err(error) => Err(unremoved(path, &error.to_string())),
    }
}

/// The directory `path` beneath `within`, held open, each step reached through no link
/// and made first where `making` says so; nothing where a step is not there and is not to
/// be made.
fn walked(
    path: &Path,
    within: &Path,
    kept: Kept,
    making: bool,
) -> Result<Option<OwnedFd>, Refusal> {
    let opening = OFlags::DIRECTORY | OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC;
    let refused = |error: Errno| unmade(path, within, &linked(error));
    if making {
        crate::fs::directory(within, true, kept.directory)
            .map_err(|error| unmade(path, within, &error.to_string()))?;
    }
    let mut here = match openat(CWD, within, opening, Mode::empty()) {
        Ok(here) => here,
        Err(Errno::NOENT) if !making => return Ok(None),
        Err(error) => return Err(refused(error)),
    };
    for name in steps(path, within)? {
        if making {
            match mkdirat(&here, name, permitted(kept.directory)) {
                Ok(()) | Err(Errno::EXIST) => {}
                Err(error) => return Err(refused(error)),
            }
        }
        here = match openat(&here, name, opening | OFlags::NOFOLLOW, Mode::empty()) {
            Ok(next) => next,
            Err(Errno::NOENT) if !making => return Ok(None),
            Err(error) => return Err(refused(error)),
        };
        if kept.owned {
            owned(&here).map_err(|why| unmade(path, within, why))?;
        }
    }
    Ok(Some(here))
}

/// Whether the directory `here` holds is the writer's own.
fn owned(here: &OwnedFd) -> Result<(), &'static str> {
    fstat(here)
        .is_ok_and(|stat| stat.st_uid == rustix::process::geteuid().as_raw())
        .then_some(())
        .ok_or("it is somebody else's")
}

/// What a step refused for, in words: a link where a directory or file was expected, or
/// the platform's own.
fn linked(error: Errno) -> String {
    match error {
        Errno::LOOP | Errno::NOTDIR => {
            "a link or something other than a directory stands on the way".to_owned()
        }
        other => other.to_string(),
    }
}

/// A failure in the platform's own words.
fn worded(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// The refusal of a directory on the way to `path` beneath `within`.
fn unmade(path: &Path, within: &Path, why: &str) -> Refusal {
    Refusal::Said(format!(
        "{} was not reached beneath {}: {why}, and lemonfiber never writes through a link \
         or into somebody else's directory",
        path.display(),
        within.display()
    ))
}

/// Make the directory `names` lead to beneath `root`, with `mode`, each step made where
/// it is missing and opened through the handle of the one above it, never through a
/// link.
///
/// # Errors
///
/// Where a step is a link or is refused.
pub fn unpacked_directory(root: &Path, names: &[&OsStr], mode: u32) -> Result<(), Refusal> {
    unpacked_walk(root, names, mode).map(drop)
}

/// Make the file `last` in the directory `above` leads to beneath `root`, new, with
/// `mode`, holding `contents`.
///
/// # Errors
///
/// Where a step is a link, something already stands at its name, or the write fails.
pub fn unpacked_file(
    root: &Path,
    above: &[&OsStr],
    last: &OsStr,
    mode: u32,
    contents: &[u8],
) -> Result<(), Refusal> {
    let names = [above, &[last]].concat();
    let here = unpacked_walk(root, above, PERMISSION_BITS)?;
    openat(
        &here,
        last,
        OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        permitted(mode),
    )
    .map(File::from)
    .map_err(std::io::Error::from)
    .and_then(|mut opened| opened.write_all(contents))
    .map_err(|error| unpacked(root, &names, &error.to_string()))
}

/// The directory `names` lead to beneath `root`, each step made where it is missing and
/// opened through the handle of the one above it, never through a link.
fn unpacked_walk(root: &Path, names: &[&OsStr], mode: u32) -> Result<OwnedFd, Refusal> {
    let opening = OFlags::DIRECTORY | OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC;
    let unmade = |error: Errno| unpacked(root, names, &error.to_string());
    let mut here = openat(CWD, root, opening | OFlags::NOFOLLOW, Mode::empty()).map_err(unmade)?;
    for name in names {
        match mkdirat(&here, *name, permitted(mode)) {
            Ok(()) | Err(Errno::EXIST) => {}
            Err(error) => return Err(unmade(error)),
        }
        here = openat(&here, *name, opening | OFlags::NOFOLLOW, Mode::empty()).map_err(unmade)?;
    }
    Ok(here)
}

/// `mode` as the permission bits a file or directory is made with, none beyond
/// [`PERMISSION_BITS`].
fn permitted(mode: u32) -> Mode {
    let bits = RawMode::try_from(mode & PERMISSION_BITS).unwrap_or_default();
    Mode::from_bits_truncate(bits)
}

#[cfg(test)]
mod tests;
