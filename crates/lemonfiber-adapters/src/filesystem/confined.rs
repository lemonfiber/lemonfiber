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
//! How each step is reached on this platform is `lemonfiber_platform::beneath`'s; what a
//! refusal means for the port is decided here.
//!
//! Synchronous, because the stack's own files are written out by code with nothing to
//! await; the asynchronous read runs the same function on a thread that may block.

use std::io::{Read as _, Write as _};
use std::path::Path;

use lemonfiber_platform::beneath::{self, Kept, Lies, Refusal};
use lemonfiber_platform::fs;
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
        match beneath::lies(path, within) {
            Lies::Plain => Lying::Plain,
            Lies::Absent => Lying::Absent,
            Lies::Escaped => Lying::Escaped,
        }
    }

    fn moved(&self, from: &Path, to: &Path, within: &Path) -> Result<(), Fault> {
        beneath::moved(from, to, within).map_err(|refusal| fault(refusal, to, within))
    }

    fn made(&self, path: &Path, within: &Path, readable: Readable) -> Result<(), Fault> {
        beneath::made(path, within, kept(readable)).map_err(|refusal| fault(refusal, path, within))
    }

    fn replaced(
        &self,
        path: &Path,
        within: &Path,
        contents: &[u8],
        readable: Readable,
    ) -> Result<(), Fault> {
        beneath::replaced(path, within, contents, kept(readable))
            .map_err(|refusal| fault(refusal, path, within))
    }

    fn bytes(&self, path: &Path, within: &Path) -> Result<Option<Vec<u8>>, Fault> {
        beneath::bytes(path, within, READ_LIMIT).map_err(|refusal| fault(refusal, path, within))
    }

    fn removed(&self, path: &Path, within: &Path) -> Result<(), Fault> {
        beneath::removed(path, within).map_err(|refusal| fault(refusal, path, within))
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
    let file = beneath::created(path, within, kept(readable).file)
        .map_err(|refusal| fault(refusal, path, within))?;
    if standing(&file, path, within) != Standing::Plain {
        return Err(Fault::escaped(path, within));
    }
    landed(
        fs::set_mode(&file, kept(readable).file)
            .and_then(|()| file.set_len(0))
            .and_then(|()| (&file).write_all(contents)),
    )
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
    let plain = opened.is_file() && named.is_ok_and(|named| fs::same_file(&opened, &named));
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
/// refusing to wait, where the platform has flags for either; where it has none, a link at
/// the last name is caught by the file it opened not being the one the name resolves to.
///
/// Not waiting matters as much as not following: a pipe put where the file is expected
/// would otherwise hold the open until something wrote to it, or read from it. A plain
/// file opens the same either way, and creating one through a link at its name is refused
/// with the rest.
fn options(purpose: Use) -> std::fs::OpenOptions {
    let mut options = std::fs::OpenOptions::new();
    match purpose {
        Use::Read => options.read(true),
        Use::Write => options.write(true).create(true),
    };
    fs::unfollowed_unwaiting(&mut options);
    options
}

/// Make `path` and every directory missing above it beneath `within`, never through a
/// link.
pub(super) fn make(path: &Path, within: &Path) -> Result<(), Fault> {
    beneath::make(path, within).map_err(|refusal| fault(refusal, path, within))
}

/// What a refusal beneath `within` means for `path`, in the port's words.
fn fault(refusal: Refusal, path: &Path, within: &Path) -> Fault {
    match refusal {
        Refusal::Escaped => Fault::escaped(path, within),
        Refusal::Said(message) => Fault::new(message),
    }
}

/// What a file and the directories above it are made with, as `readable` says.
const fn kept(readable: Readable) -> Kept {
    match readable {
        Readable::Owner => Kept::OWNER,
        Readable::Private => Kept::PRIVATE,
        Readable::Shared => Kept::SHARED,
    }
}

mod unpacking;
pub(crate) use unpacking::{unpacked, UNPACKED_AT_MOST};

#[cfg(test)]
mod tests;
