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

use lemonfiber_platform::beneath;
use lemonfiber_platform::fs::PERMISSION_BITS;
use lemonfiber_ports::filesystem::Fault;

/// The most an archive may hold once unpacked, so a volume that is not what it should be
/// cannot fill the disk it is copied onto.
pub(crate) const UNPACKED_AT_MOST: u64 = 1 << 30;

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
        let mode = header.mode().map_err(unreadable)? & PERMISSION_BITS;
        let path = entry.path().map_err(unreadable)?.into_owned();
        let Some(names) = archived(&path) else {
            return Err(stepping_out(&path));
        };
        let Some((last, inside)) = names.split_last() else {
            continue;
        };
        let above = [below.as_slice(), inside].concat();
        if kind.is_dir() {
            beneath::unpacked_directory(within, &[above.as_slice(), &[*last]].concat(), mode)
                .map_err(|refusal| super::fault(refusal, into, within))?;
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
            beneath::unpacked_file(within, &above, last, mode, &contents)
                .map_err(|refusal| super::fault(refusal, into, within))?;
        }
    }
    Ok(())
}

/// The names `path` steps through below what was archived, or nothing where it steps
/// anywhere but down.
fn archived(path: &Path) -> Option<Vec<&std::ffi::OsStr>> {
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

#[cfg(test)]
mod tests;
