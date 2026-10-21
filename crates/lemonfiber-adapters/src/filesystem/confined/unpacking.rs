//! Writing an archive the engine handed over into a directory of a service's own.
//!
//! The archive is the engine's answer, and what a container left in a volume is in it, so
//! nothing in it is trusted to say where it goes. Every entry is written beneath the one
//! directory it was asked for, each directory on the way made and opened through the
//! handle of the one above it and never through a link, and each file made new there,
//! refusing whatever already stands at its name. Only plain files and directories are
//! written, with the permission bits the archive gives them and no others: a link, a
//! device or a pipe in a volume is state no service needs moved, and is left behind.

use std::io::Read as _;
use std::path::{Component, Path};

use lemonfiber_ports::filesystem::Fault;

/// The most an archive may hold once unpacked, so a volume that is not what it should be
/// cannot fill the disk it is copied onto.
pub(crate) const UNPACKED_AT_MOST: u64 = 1 << 30;

/// The permission bits a written file or directory may keep: none of set-user, set-group
/// or sticky.
const PERMITTED: u32 = 0o777;

/// Write every plain file and directory of the tar `archive` beneath `into`, which lies
/// beneath `within`, dropping the archive's first step: the name of what was archived.
///
/// # Errors
///
/// Where `into` is not a directory beneath `within` reached through no link, an entry
/// steps outside it or meets something already there, or the archive holds more than
/// [`UNPACKED_AT_MOST`].
pub(crate) fn unpacked(archive: &[u8], into: &Path, within: &Path) -> Result<(), Fault> {
    bounded(archive, into, within, UNPACKED_AT_MOST)
}

/// The same, refusing an archive whose files hold more than `at_most` bytes.
fn bounded(archive: &[u8], into: &Path, within: &Path, at_most: u64) -> Result<(), Fault> {
    super::make(into, within)?;
    let below = super::steps(into, within)?;
    let mut entries = tar::Archive::new(archive);
    let mut written: u64 = 0;
    for entry in entries.entries().map_err(unreadable)? {
        let mut entry = entry.map_err(unreadable)?;
        let header = entry.header();
        let kind = header.entry_type();
        let mode = header.mode().map_err(unreadable)? & PERMITTED;
        let path = entry.path().map_err(unreadable)?.into_owned();
        let Some(names) = beneath(&path) else {
            return Err(stepping_out(&path));
        };
        let Some((last, inside)) = names.split_last() else {
            continue;
        };
        let above = [below.as_slice(), inside].concat();
        if kind.is_dir() {
            directory(within, &[above.as_slice(), &[*last]].concat(), mode)?;
        } else if kind.is_file() {
            written = written.saturating_add(entry.size());
            if written > at_most {
                return Err(Fault::new(format!(
                    "what was handed over holds more than {at_most} bytes, and lemonfiber \
                     copies no more than that"
                )));
            }
            let mut contents = Vec::new();
            entry.read_to_end(&mut contents).map_err(unreadable)?;
            file(within, &above, last, mode, &contents)?;
        }
    }
    Ok(())
}

/// The names `path` steps through below what was archived, or nothing where it steps
/// anywhere but down.
fn beneath(path: &Path) -> Option<Vec<&std::ffi::OsStr>> {
    let mut names = Vec::new();
    for (at, part) in path.components().enumerate() {
        match part {
            Component::Normal(name) if at > 0 => names.push(name),
            Component::Normal(_) | Component::CurDir => {}
            _ => return None,
        }
    }
    Some(names)
}

/// Make the directory `names` lead to beneath `root`, with `mode`.
#[cfg(unix)]
fn directory(root: &Path, names: &[&std::ffi::OsStr], mode: u32) -> Result<(), Fault> {
    walked(root, names, mode).map(|_| ())
}

/// Make the file `last` in the directory `above` leads to beneath `root`, new, with
/// `mode`, holding `contents`.
#[cfg(unix)]
fn file(
    root: &Path,
    above: &[&std::ffi::OsStr],
    last: &std::ffi::OsStr,
    mode: u32,
    contents: &[u8],
) -> Result<(), Fault> {
    use rustix::fs::{openat, OFlags};
    use std::io::Write as _;
    let names = [above, &[last]].concat();
    let here = walked(root, above, PERMITTED)?;
    openat(
        &here,
        last,
        OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        permitted(mode),
    )
    .map(std::fs::File::from)
    .map_err(std::io::Error::from)
    .and_then(|mut opened| opened.write_all(contents))
    .map_err(|error| unwritten(root, &names, &error.to_string()))
}

/// The directory `names` lead to beneath `root`, each step made where it is missing and
/// opened through the handle of the one above it, never through a link.
#[cfg(unix)]
fn walked(
    root: &Path,
    names: &[&std::ffi::OsStr],
    mode: u32,
) -> Result<rustix::fd::OwnedFd, Fault> {
    use rustix::fs::{mkdirat, openat, Mode, OFlags, CWD};
    let opening = OFlags::DIRECTORY | OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC;
    let unmade = |error: rustix::io::Errno| unwritten(root, names, &error.to_string());
    let mut here = openat(CWD, root, opening | OFlags::NOFOLLOW, Mode::empty()).map_err(unmade)?;
    for name in names {
        match mkdirat(&here, *name, permitted(mode)) {
            Ok(()) | Err(rustix::io::Errno::EXIST) => {}
            Err(error) => return Err(unmade(error)),
        }
        here = openat(&here, *name, opening | OFlags::NOFOLLOW, Mode::empty()).map_err(unmade)?;
    }
    Ok(here)
}

/// `mode` as the permission bits a file or directory is made with, none beyond
/// [`PERMITTED`].
#[cfg(unix)]
fn permitted(mode: u32) -> rustix::fs::Mode {
    let bits = rustix::fs::RawMode::try_from(mode & PERMITTED).unwrap_or_default();
    rustix::fs::Mode::from_bits_truncate(bits)
}

/// Make the directory `names` lead to beneath `root`. Windows has no flag that refuses a
/// link on the way, so the making is [`super::make`]'s, which looks at each step first.
#[cfg(windows)]
fn directory(root: &Path, names: &[&std::ffi::OsStr], _mode: u32) -> Result<(), Fault> {
    let path = names
        .iter()
        .fold(root.to_path_buf(), |at, name| at.join(name));
    super::make(&path, root)
}

/// Make the file `last` in the directory `above` leads to beneath `root`, new, holding
/// `contents`.
#[cfg(windows)]
fn file(
    root: &Path,
    above: &[&std::ffi::OsStr],
    last: &std::ffi::OsStr,
    _mode: u32,
    contents: &[u8],
) -> Result<(), Fault> {
    use std::io::Write as _;
    let names = [above, &[last]].concat();
    let names = names.as_slice();
    directory(root, above, PERMITTED)?;
    let path = names
        .iter()
        .fold(root.to_path_buf(), |at, name| at.join(name));
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .and_then(|mut made| made.write_all(contents))
        .map_err(|error| unwritten(root, names, &error.to_string()))
}

/// The refusal of an archive that could not be read.
fn unreadable(error: impl std::fmt::Display) -> Fault {
    Fault::new(format!("what was handed over could not be read: {error}"))
}

/// The refusal of an entry that names somewhere other than beneath what was archived.
fn stepping_out(named: &Path) -> Fault {
    Fault::new(format!(
        "what was handed over names {}, which does not lie beneath what was copied, and \
         lemonfiber writes nothing outside it",
        named.display()
    ))
}

/// The refusal of writing what `names` lead to beneath `root`, saying `why`.
fn unwritten(root: &Path, names: &[&std::ffi::OsStr], why: &str) -> Fault {
    let path = names
        .iter()
        .fold(root.to_path_buf(), |at, name| at.join(name));
    Fault::new(format!("{} was not written: {why}", path.display()))
}

#[cfg(test)]
mod tests;
