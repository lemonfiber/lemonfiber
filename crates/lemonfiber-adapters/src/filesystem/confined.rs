//! Reading and writing a file, and making a directory, somebody else's container can
//! write.
//!
//! The container owns the directory, so it can put a link where a file is expected, link
//! a directory on the way to one, or leave a pipe there. A plain open follows the first
//! two anywhere on the host and waits on the third for ever. So every file here is opened
//! once, refusing a link at its own name and refusing to wait, and then checked by what
//! was opened rather than by its name: the name, resolved, has to lie beneath `within` and
//! to name the very file the handle holds. A link swapped in on the way after the open
//! resolves somewhere else, or names another file, and either is refused — so there is no
//! moment between a look and a use for it to change under.
//!
//! A move is made the same way: every step to either end is reached through no link.
//!
//! Synchronous, because the stack's own files are written out by code with nothing to
//! await; the asynchronous read runs the same function on a thread that may block.

use std::io::{Read as _, Write as _};
use std::path::{Component, Path};

use lemonfiber_ports::filesystem::{Beneath, Confined, Fault, Lying, Readable, READ_LIMIT};

use super::Disk;

impl Confined for Disk {
    fn read(&self, path: &Path, within: &Path) -> Beneath {
        read(path, within)
    }

    fn overwrite(&self, path: &Path, within: &Path, contents: &[u8]) -> Result<(), Fault> {
        overwrite(path, within, contents)
    }

    fn written(
        &self,
        path: &Path,
        within: &Path,
        contents: &[u8],
        readable: Readable,
    ) -> Result<(), Fault> {
        written(path, within, contents, readable)
    }

    fn lies(&self, path: &Path, within: &Path) -> Lying {
        lies(path, within)
    }

    fn moved(&self, from: &Path, to: &Path, within: &Path) -> Result<(), Fault> {
        moving::moved(from, to, within)
    }

    fn made(&self, path: &Path, within: &Path, readable: Readable) -> Result<(), Fault> {
        replacing::made(path, within, readable)
    }

    fn replaced(
        &self,
        path: &Path,
        within: &Path,
        contents: &[u8],
        readable: Readable,
    ) -> Result<(), Fault> {
        replacing::replaced(path, within, contents, readable)
    }

    fn bytes(&self, path: &Path, within: &Path) -> Result<Option<Vec<u8>>, Fault> {
        replacing::bytes(path, within)
    }
}

/// What an opened handle turned out to be, against the name it was opened by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Standing {
    /// A plain file, beneath the directory, and the one the name resolves to.
    Plain,
    /// Something else: a link on the way out, another file, a pipe, a directory.
    Escaped,
    /// The name or the directory no longer resolves, so there is nothing to compare.
    Gone,
}

/// Read `path`, a plain file beneath `within` no larger than [`READ_LIMIT`].
pub(super) fn read(path: &Path, within: &Path) -> Beneath {
    let Ok(file) = options(Use::Read).open(path) else {
        return unopened(path);
    };
    match standing(&file, path, within) {
        Standing::Plain => {}
        Standing::Escaped => return Beneath::Escaped,
        Standing::Gone => return Beneath::Absent,
    }
    // One byte past the limit is asked for, so a file that grew past it after the check
    // above is told from one that stopped exactly at it.
    let mut bytes = Vec::new();
    let read = (&file).take(READ_LIMIT + 1).read_to_end(&mut bytes);
    match (read, String::from_utf8(bytes)) {
        (Ok(length), _) if length as u64 > READ_LIMIT => Beneath::Escaped,
        (Ok(_), Ok(text)) => Beneath::Read(text),
        _ => Beneath::Absent,
    }
}

/// Write `contents` into `path` in place, creating it where it is not there, where it is
/// a plain file beneath `within`.
///
/// Emptied only once it is known to be that file, through the handle that was checked,
/// so nothing a link points at is ever truncated. A link to a directory on the way can
/// still have the create leave a new, empty file where it leads; nothing is written into
/// it, and the write is refused.
pub(super) fn overwrite(path: &Path, within: &Path, contents: &[u8]) -> Result<(), Fault> {
    let file = match options(Use::Write).open(path) {
        Ok(file) => file,
        Err(_) if unopened(path) == Beneath::Escaped => return Err(Fault::escaped(path, within)),
        Err(error) => return Err(Fault::new(error.to_string())),
    };
    if standing(&file, path, within) != Standing::Plain {
        return Err(Fault::escaped(path, within));
    }
    landed(file.set_len(0).and_then(|()| (&file).write_all(contents)))
}

/// Write `contents` whole into `path`, made with every directory missing above it, as
/// `readable` says, opened through the handle of the directory it is in and refusing a
/// link at its own name, then held to lying beneath `within` by what was opened.
pub(super) fn written(
    path: &Path,
    within: &Path,
    contents: &[u8],
    readable: Readable,
) -> Result<(), Fault> {
    let file = created(path, within, readable)?;
    if standing(&file, path, within) != Standing::Plain {
        return Err(Fault::escaped(path, within));
    }
    landed(
        kept_to(&file, readable)
            .and_then(|()| file.set_len(0))
            .and_then(|()| (&file).write_all(contents)),
    )
}

/// `path` opened to write, made new readable as `readable` says where it is not there,
/// through the handle of the directory it is in, every step to which was reached through
/// no link, and refusing a link at its own name.
#[cfg(unix)]
fn created(path: &Path, within: &Path, readable: Readable) -> Result<std::fs::File, Fault> {
    use rustix::fs::{openat, Mode, OFlags};
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(Fault::escaped(path, within));
    };
    let here = made(parent, within)?;
    let flags =
        OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
    let made_as = match readable {
        Readable::Owner | Readable::Private => Mode::RUSR | Mode::WUSR,
        Readable::Shared => Mode::RUSR | Mode::WUSR | Mode::RGRP | Mode::ROTH,
    };
    openat(&here, name, flags, made_as)
        .map(std::fs::File::from)
        .map_err(|error| match error {
            rustix::io::Errno::LOOP => Fault::escaped(path, within),
            error => Fault::new(error.to_string()),
        })
}

/// `path` opened to write, made where it is not there with every directory above it.
/// Windows has no flag that refuses a link on the way, so what was opened is held to
/// `within` afterwards.
#[cfg(windows)]
fn created(path: &Path, within: &Path, _readable: Readable) -> Result<std::fs::File, Fault> {
    if let Some(parent) = path.parent() {
        make(parent, within)?;
    }
    options(Use::Write)
        .open(path)
        .map_err(|error| Fault::new(error.to_string()))
}

/// The mode a file readable as `readable` says is held to.
#[cfg(unix)]
const fn mode(readable: Readable) -> u32 {
    match readable {
        Readable::Owner | Readable::Private => 0o600,
        Readable::Shared => 0o644,
    }
}

/// `file` held to the mode `readable` names, through its own handle, whatever it was made
/// with.
#[cfg(unix)]
fn kept_to(file: &std::fs::File, readable: Readable) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    file.set_permissions(std::fs::Permissions::from_mode(mode(readable)))
}

/// Nothing, where the platform keeps no mode.
#[cfg(windows)]
fn kept_to(_file: &std::fs::File, _readable: Readable) -> std::io::Result<()> {
    Ok(())
}

/// Make `path` and every directory missing above it, one at a time from `within` down,
/// each made and then opened through the handle of the one above it and never through
/// a link, so a link put on the way between two steps is refused rather than followed.
#[cfg(unix)]
pub(super) fn make(path: &Path, within: &Path) -> Result<(), Fault> {
    made(path, within).map(drop)
}

/// The directory `path`, made with every directory missing above it as [`make`] makes
/// them, held open by the handle the walk ended on.
#[cfg(unix)]
fn made(path: &Path, within: &Path) -> Result<rustix::fd::OwnedFd, Fault> {
    use rustix::fs::{mkdirat, openat, Mode, OFlags, CWD};
    let opening = OFlags::DIRECTORY | OFlags::RDONLY | OFlags::CLOEXEC;
    let unmade = |error: rustix::io::Errno| {
        Fault::new(format!(
            "{} was not made beneath {}: {error}",
            path.display(),
            within.display()
        ))
    };
    let mut here = openat(CWD, within, opening, Mode::empty()).map_err(unmade)?;
    for name in steps(path, within)? {
        match mkdirat(&here, name, Mode::RWXU | Mode::RWXG | Mode::RWXO) {
            Ok(()) | Err(rustix::io::Errno::EXIST) => {}
            Err(error) => return Err(unmade(error)),
        }
        here = openat(&here, name, opening | OFlags::NOFOLLOW, Mode::empty()).map_err(unmade)?;
    }
    Ok(here)
}

/// Make `path` and every directory missing above it, one at a time from `within` down.
/// Windows has no flag that refuses a link on the way, so each step that is already
/// there is looked at first and refused where it is not a directory of its own.
#[cfg(windows)]
pub(super) fn make(path: &Path, within: &Path) -> Result<(), Fault> {
    let mut here = within.to_path_buf();
    for name in steps(path, within)? {
        here.push(name);
        match std::fs::symlink_metadata(&here) {
            Ok(found) if found.is_dir() => {}
            Ok(_) => {
                return Err(Fault::new(format!(
                    "{} is a link or a file, and lemonfiber makes its own directory there \
                     rather than following one",
                    here.display()
                )))
            }
            Err(_) => std::fs::create_dir(&here).map_err(|error| Fault::new(error.to_string()))?,
        }
    }
    Ok(())
}

/// Where `path` lies beneath `within`: each directory on the way opened through the handle
/// of the one above it, never through a link and never waiting on what it opens, and the
/// last step only looked at, so a pipe or a device put there is never opened.
#[cfg(unix)]
pub(super) fn lies(path: &Path, within: &Path) -> Lying {
    use rustix::fs::{openat, statat, AtFlags, FileType, Mode, OFlags, CWD};
    let Ok(mut names) = steps(path, within) else {
        return Lying::Escaped;
    };
    let opening = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK | OFlags::DIRECTORY;
    let Ok(mut here) = openat(CWD, within, opening, Mode::empty()) else {
        return Lying::Absent;
    };
    let Some(last) = names.pop() else {
        return Lying::Plain;
    };
    for name in names {
        match openat(&here, name, opening | OFlags::NOFOLLOW, Mode::empty()) {
            Ok(opened) => here = opened,
            Err(rustix::io::Errno::NOENT) => return Lying::Absent,
            Err(_) => return Lying::Escaped,
        }
    }
    match statat(&here, last, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(found) if FileType::from_raw_mode(found.st_mode) == FileType::Symlink => Lying::Escaped,
        Ok(_) => Lying::Plain,
        Err(rustix::io::Errno::NOENT) => Lying::Absent,
        Err(_) => Lying::Escaped,
    }
}

/// Where `path` lies beneath `within`, each step looked at and refused where it is a link.
#[cfg(windows)]
pub(super) fn lies(path: &Path, within: &Path) -> Lying {
    let Ok(names) = steps(path, within) else {
        return Lying::Escaped;
    };
    let mut here = within.to_path_buf();
    for name in names {
        here.push(name);
        match std::fs::symlink_metadata(&here) {
            Ok(found) if found.file_type().is_symlink() => return Lying::Escaped,
            Ok(_) => {}
            Err(_) => return Lying::Absent,
        }
    }
    Lying::Plain
}

/// The names `path` is made of below `within`, refused where it is not beneath it or
/// steps back up out of it.
fn steps<'a>(path: &'a Path, within: &Path) -> Result<Vec<&'a std::ffi::OsStr>, Fault> {
    let rest = path
        .strip_prefix(within)
        .map_err(|_| outside(path, within))?;
    rest.components()
        .map(|part| match part {
            Component::Normal(name) => Ok(name),
            _ => Err(outside(path, within)),
        })
        .collect()
}

/// The refusal of a directory that does not lie beneath the one it is made in.
fn outside(path: &Path, within: &Path) -> Fault {
    Fault::new(format!(
        "{} does not lie beneath {}, and lemonfiber makes its own directories only there",
        path.display(),
        within.display()
    ))
}

/// What emptying and writing the checked file came to, a failure in the platform's own
/// words.
///
/// Its own function because a write that fails part-way through a plain file — a disk
/// filling — cannot be staged; handed the answer directly, it is an ordinary test.
fn landed(written: std::io::Result<()>) -> Result<(), Fault> {
    written.map_err(|error| Fault::new(error.to_string()))
}

/// Whether the file `file` holds is the plain file `path` names, beneath `within`.
fn standing(file: &std::fs::File, path: &Path, within: &Path) -> Standing {
    let (Ok(opened), Ok(root), Ok(resolved)) = (
        file.metadata(),
        std::fs::canonicalize(within),
        std::fs::canonicalize(path),
    ) else {
        return Standing::Gone;
    };
    let named = std::fs::metadata(&resolved);
    let plain = opened.is_file() && named.is_ok_and(|named| same_file(&opened, &named));
    if plain && resolved.starts_with(&root) {
        Standing::Plain
    } else {
        Standing::Escaped
    }
}

/// Why a file that would not open was not reached: a link at its last name, which the
/// open refused to follow, or nothing there to open at all.
pub(super) fn unopened(path: &Path) -> Beneath {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Beneath::Escaped,
        Ok(_) | Err(_) => Beneath::Absent,
    }
}

/// What a file is opened for.
#[derive(Debug, Clone, Copy)]
enum Use {
    /// Reading what is there.
    Read,
    /// Writing over it, or creating it where nothing is there.
    Write,
}

/// Opening for `purpose`, refusing to follow a link at the last name of the path and
/// refusing to wait.
///
/// Not waiting matters as much as not following: a pipe put where the file is expected
/// would otherwise hold the open until something wrote to it, or read from it. A plain
/// file opens the same either way, and creating one through a link at its name is refused
/// with the rest.
#[cfg(unix)]
fn options(purpose: Use) -> std::fs::OpenOptions {
    use std::os::unix::fs::OpenOptionsExt as _;
    let mut options = std::fs::OpenOptions::new();
    match purpose {
        Use::Read => options.read(true),
        Use::Write => options.write(true).create(true),
    };
    options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    options
}

/// Opening for `purpose`. Windows has no flag for it, so a link at the last name is
/// caught by the file it opened not being the one the name resolves to.
#[cfg(windows)]
fn options(purpose: Use) -> std::fs::OpenOptions {
    let mut options = std::fs::OpenOptions::new();
    match purpose {
        Use::Read => options.read(true),
        Use::Write => options.write(true).create(true),
    };
    options
}

/// Whether two readings are of one file: the same device and the same number on it.
#[cfg(unix)]
fn same_file(one: &std::fs::Metadata, other: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;

    one.dev() == other.dev() && one.ino() == other.ino()
}

/// Whether two readings are of one file: the same volume and the same index on it.
#[cfg(windows)]
fn same_file(one: &std::fs::Metadata, other: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;

    one.volume_serial_number() == other.volume_serial_number()
        && one.file_index() == other.file_index()
        && one.file_index().is_some()
}

mod moving;
mod replacing;
mod unpacking;

pub(crate) use unpacking::{unpacked, UNPACKED_AT_MOST};

#[cfg(test)]
mod tests;
