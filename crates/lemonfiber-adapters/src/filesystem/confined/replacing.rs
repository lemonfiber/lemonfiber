//! Making a directory, replacing a file whole and reading one back, beneath a directory
//! somebody else's container can write.
//!
//! Every step from `within` down is opened through the handle of the one above it, never
//! through a link and never waiting on what it opens, and the file itself is reached only
//! through the handle of the directory it is in. A file is replaced by writing a new one
//! beside it, made new so nothing already standing at that name is written through,
//! flushing it, and renaming it over the old one, so a link planted at its name is
//! replaced rather than followed and a reader never sees half of it.

use std::path::Path;

use lemonfiber_ports::filesystem::{Fault, Readable, READ_LIMIT};

use super::steps;

/// The directory `path`, made where it is not there.
#[cfg(unix)]
pub(super) fn made(path: &Path, within: &Path, readable: Readable) -> Result<(), Fault> {
    walked(path, within, readable, true).map(drop)
}

/// `contents` written beside `path` and renamed over it.
#[cfg(unix)]
pub(super) fn replaced(
    path: &Path,
    within: &Path,
    contents: &[u8],
    readable: Readable,
) -> Result<(), Fault> {
    use rustix::fs::{fsync, openat, renameat, unlinkat, AtFlags, Mode, OFlags};
    use std::io::Write as _;
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(unwritten(path, "it names no file"));
    };
    let here = walked(parent, within, readable, true)?
        .ok_or(unwritten(path, "the directory it is in could not be made"))?;
    let staging = staging(name);
    let made_as = match readable {
        Readable::Owner | Readable::Private => Mode::RUSR | Mode::WUSR,
        Readable::Shared => Mode::RUSR | Mode::WUSR | Mode::RGRP | Mode::ROTH,
    };
    let created = OFlags::RDWR
        | OFlags::CREATE
        | OFlags::EXCL
        | OFlags::NOFOLLOW
        | OFlags::NONBLOCK
        | OFlags::CLOEXEC;
    let _ = unlinkat(&here, &staging, AtFlags::empty());
    let file = openat(&here, &staging, created, made_as)
        .map(std::fs::File::from)
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

/// What `path` holds, reached through no link.
#[cfg(unix)]
pub(super) fn bytes(path: &Path, within: &Path) -> Result<Option<Vec<u8>>, Fault> {
    use rustix::fs::{fstat, openat, FileType, Mode, OFlags};
    use std::io::Read as _;
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(unread(path, "it names no file"));
    };
    let Some(here) = walked(parent, within, Readable::Shared, false)? else {
        return Ok(None);
    };
    let opening = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
    let file = match openat(&here, name, opening, Mode::empty()) {
        Ok(file) => file,
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(error) => return Err(unread(path, &linked(error))),
    };
    let plain = fstat(&file)
        .is_ok_and(|stat| FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile);
    if !plain {
        return Err(unread(path, "it is not a plain file"));
    }
    let mut held = Vec::new();
    let read = std::fs::File::from(file)
        .take(READ_LIMIT + 1)
        .read_to_end(&mut held)
        .ok();
    match read {
        Some(length) if length as u64 <= READ_LIMIT => Ok(Some(held)),
        _ => Err(unread(
            path,
            "it could not be read whole, or is larger than lemonfiber reads",
        )),
    }
}

/// Take away the file `path`, through the handle of the directory it is in.
#[cfg(unix)]
pub(super) fn removed(path: &Path, within: &Path) -> Result<(), Fault> {
    use rustix::fs::{unlinkat, AtFlags};
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(unremoved(path, "it names no file"));
    };
    let Some(here) = walked(parent, within, Readable::Shared, false)? else {
        return Ok(());
    };
    match unlinkat(&here, name, AtFlags::empty()) {
        Ok(()) | Err(rustix::io::Errno::NOENT) => Ok(()),
        Err(error) => Err(unremoved(path, &error.to_string())),
    }
}

/// The directory `path` beneath `within`, held open, each step reached through no link
/// and made first where `making` says so; nothing where a step is not there and is not to
/// be made.
#[cfg(unix)]
fn walked(
    path: &Path,
    within: &Path,
    readable: Readable,
    making: bool,
) -> Result<Option<rustix::fd::OwnedFd>, Fault> {
    use rustix::fs::{mkdirat, openat, Mode, OFlags, CWD};
    let opening = OFlags::DIRECTORY | OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC;
    let made_as = match readable {
        Readable::Owner | Readable::Private => Mode::RWXU,
        Readable::Shared => Mode::RWXU | Mode::RWXG | Mode::RWXO,
    };
    let refused = |error: rustix::io::Errno| unmade(path, within, &linked(error));
    if making {
        use std::os::unix::fs::DirBuilderExt as _;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(match readable {
                Readable::Owner | Readable::Private => 0o700,
                Readable::Shared => 0o777,
            })
            .create(within)
            .map_err(|error| unmade(path, within, &error.to_string()))?;
    }
    let mut here = match openat(CWD, within, opening, Mode::empty()) {
        Ok(here) => here,
        Err(rustix::io::Errno::NOENT) if !making => return Ok(None),
        Err(error) => return Err(refused(error)),
    };
    for name in steps(path, within)? {
        if making {
            match mkdirat(&here, name, made_as) {
                Ok(()) | Err(rustix::io::Errno::EXIST) => {}
                Err(error) => return Err(refused(error)),
            }
        }
        here = match openat(&here, name, opening | OFlags::NOFOLLOW, Mode::empty()) {
            Ok(next) => next,
            Err(rustix::io::Errno::NOENT) if !making => return Ok(None),
            Err(error) => return Err(refused(error)),
        };
        if readable == Readable::Owner {
            owned(&here).map_err(|why| unmade(path, within, why))?;
        }
    }
    Ok(Some(here))
}

/// Whether the directory `here` holds is the writer's own.
#[cfg(unix)]
fn owned(here: &rustix::fd::OwnedFd) -> Result<(), &'static str> {
    rustix::fs::fstat(here)
        .is_ok_and(|stat| stat.st_uid == rustix::process::geteuid().as_raw())
        .then_some(())
        .ok_or("it is somebody else's")
}

/// What a step refused for, in words: a link where a directory or file was expected, or
/// the platform's own.
#[cfg(unix)]
fn linked(error: rustix::io::Errno) -> String {
    match error {
        rustix::io::Errno::LOOP | rustix::io::Errno::NOTDIR => {
            "a link or something other than a directory stands on the way".to_owned()
        }
        other => other.to_string(),
    }
}

/// The directory `path`, made where it is not there. Windows has no flag that refuses a
/// link on the way, so each step is looked at first and refused where it is a link.
#[cfg(windows)]
pub(super) fn made(path: &Path, within: &Path, _readable: Readable) -> Result<(), Fault> {
    super::make(path, within)
}

/// `contents` written beside `path` and renamed over it, every step looked at first.
#[cfg(windows)]
pub(super) fn replaced(
    path: &Path,
    within: &Path,
    contents: &[u8],
    readable: Readable,
) -> Result<(), Fault> {
    use std::io::Write as _;
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(unwritten(path, "it names no file"));
    };
    made(parent, within, readable)?;
    if super::lies(path, within) == lemonfiber_ports::filesystem::Lying::Escaped {
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

/// What `path` holds, every step looked at first.
#[cfg(windows)]
pub(super) fn bytes(path: &Path, within: &Path) -> Result<Option<Vec<u8>>, Fault> {
    use lemonfiber_ports::filesystem::Lying;
    match super::lies(path, within) {
        Lying::Absent => return Ok(None),
        Lying::Escaped => return Err(unread(path, "a link stands on the way")),
        Lying::Plain => {}
    }
    let held = std::fs::read(path).map_err(|error| unread(path, &error.to_string()))?;
    if held.len() as u64 > READ_LIMIT {
        return Err(unread(path, "it is larger than lemonfiber reads"));
    }
    Ok(Some(held))
}

/// A failure in the platform's own words.
#[cfg(unix)]
fn worded(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// Take away the file `path`, every step looked at first.
#[cfg(windows)]
pub(super) fn removed(path: &Path, within: &Path) -> Result<(), Fault> {
    use lemonfiber_ports::filesystem::Lying;
    match super::lies(path, within) {
        Lying::Absent => Ok(()),
        Lying::Escaped => Err(unremoved(path, "a link stands on the way")),
        Lying::Plain => {
            std::fs::remove_file(path).map_err(|error| unremoved(path, &error.to_string()))
        }
    }
}

/// The name a file is written under beside `name` before it is renamed over it.
fn staging(name: &std::ffi::OsStr) -> std::ffi::OsString {
    let mut staging = std::ffi::OsString::from(".");
    staging.push(name);
    staging.push(".writing");
    staging
}

/// The refusal of a directory on the way to `path` beneath `within`.
fn unmade(path: &Path, within: &Path, why: &str) -> Fault {
    Fault::new(format!(
        "{} was not reached beneath {}: {why}, and lemonfiber never writes through a link \
         or into somebody else's directory",
        path.display(),
        within.display()
    ))
}

/// The refusal of writing `path`.
fn unwritten(path: &Path, why: &str) -> Fault {
    Fault::new(format!("{} was not written: {why}", path.display()))
}

/// The refusal of taking `path` away.
fn unremoved(path: &Path, why: &str) -> Fault {
    Fault::new(format!("{} was not taken away: {why}", path.display()))
}

/// The refusal of reading `path`.
fn unread(path: &Path, why: &str) -> Fault {
    Fault::new(format!("{} was not read: {why}", path.display()))
}

#[cfg(test)]
mod tests;
