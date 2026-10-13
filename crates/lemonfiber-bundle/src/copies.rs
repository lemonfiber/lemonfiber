//! The copies beside the bundle, read without following a link.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{File, FileType};
use std::io::{Error, ErrorKind, Read as _};
use std::path::Path;

use crate::{Refused, EXTENSION, MANIFEST};

/// The one entry in the copies directory that is not a copy: what keeps an empty
/// directory in git.
const KEEP: &str = ".gitkeep";

/// One copy as found on disk.
#[derive(Debug, Default)]
pub(crate) struct Copy {
    /// Its name in the copies directory.
    pub(crate) name: String,
    /// Its manifest's text, where it has one.
    pub(crate) manifest: Option<String>,
    /// Every other file, by its path from the copy's root.
    pub(crate) files: BTreeMap<String, Vec<u8>>,
    /// Every path in it that is a link, which nothing reads through.
    pub(crate) links: Vec<String>,
}

/// Every copy in `directory`, by the plugin id its name gives; a missing directory holds
/// none.
///
/// At schema 1 a copy is `<id>.toml`; otherwise it is the directory `<id>/`. An entry of
/// the other kind is kept under its whole name, which no pin names.
pub(crate) fn read(directory: &Path, schema: u32) -> Result<BTreeMap<String, Copy>, Refused> {
    let entries = match listed(directory) {
        Ok(entries) => entries,
        Err(missing) if missing.kind() == ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(source) => return Err(unread(directory, source)),
    };
    let mut copies = BTreeMap::new();
    for (name, kind) in entries {
        let name = named(directory, name)?;
        if name == KEEP {
            continue;
        }
        let path = directory.join(&name);
        let copy = if kind.is_symlink() {
            Copy {
                links: vec![String::new()],
                ..Copy::default()
            }
        } else if schema == 1 && kind.is_file() && name.ends_with(EXTENSION) {
            let id = name.trim_end_matches(EXTENSION).to_owned();
            copies.insert(
                id,
                Copy {
                    name,
                    manifest: Some(text(&path)?),
                    ..Copy::default()
                },
            );
            continue;
        } else if schema != 1 && kind.is_dir() {
            walked(&path)?
        } else {
            Copy::default()
        };
        copies.insert(name.clone(), Copy { name, ..copy });
    }
    Ok(copies)
}

/// A copy's directory, every file read and every link noted rather than followed.
pub(crate) fn walked(root: &Path) -> Result<Copy, Refused> {
    let mut copy = Copy::default();
    let mut pending = vec![String::new()];
    while let Some(relative) = pending.pop() {
        let at = root.join(&relative);
        let entries = listed(&at).map_err(|source| unread(&at, source))?;
        for (name, kind) in entries {
            let name = named(&at, name)?;
            let path = if relative.is_empty() {
                name
            } else {
                format!("{relative}/{name}")
            };
            if kind.is_symlink() {
                copy.links.push(path);
            } else if kind.is_dir() {
                pending.push(path);
            } else if path == MANIFEST {
                copy.manifest = Some(text(&root.join(&path))?);
            } else {
                let bytes = bytes(&root.join(&path))?;
                copy.files.insert(path, bytes);
            }
        }
    }
    Ok(copy)
}

/// Every entry in the directory `at`, by its name, with its kind as the entry itself
/// is, a link unfollowed.
fn listed(at: &Path) -> std::io::Result<Vec<(OsString, FileType)>> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(at)? {
        let entry = entry?;
        entries.push((entry.file_name(), entry.file_type()?));
    }
    Ok(entries)
}

/// The entry `name` in the directory `at` as text, refused where it is not UTF-8 rather
/// than read lossily into a name another entry may also read as.
pub(crate) fn named(at: &Path, name: OsString) -> Result<String, Refused> {
    name.into_string().map_err(|name| {
        unread(
            &at.join(name),
            Error::new(ErrorKind::InvalidData, "the name is not UTF-8"),
        )
    })
}

/// Whether any of the components of `relative` under `root` is a symbolic link, named by
/// the first that is.
pub(crate) fn linked(root: &Path, relative: &str) -> Option<String> {
    let mut prefix = String::new();
    for component in relative.split('/') {
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(component);
        if std::fs::symlink_metadata(root.join(&prefix)).is_ok_and(|found| found.is_symlink()) {
            return Some(prefix);
        }
    }
    None
}

/// The text at `path`, read through no link.
pub(crate) fn text(path: &Path) -> Result<String, Refused> {
    String::from_utf8(bytes(path)?)
        .map_err(|unreadable| unread(path, Error::new(ErrorKind::InvalidData, unreadable)))
}

/// The bytes at `path`, read through no link and waiting on no pipe.
pub(crate) fn bytes(path: &Path) -> Result<Vec<u8>, Refused> {
    let mut bytes = Vec::new();
    match opened(path).and_then(|mut file| file.read_to_end(&mut bytes)) {
        Ok(_) => Ok(bytes),
        Err(source) => Err(unread(path, source)),
    }
}

/// The file at `path`, opened for reading through no link and waiting on no pipe.
#[cfg(unix)]
fn opened(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt as _;
    File::options()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
}

/// The file at `path`, where the platform has no flag that refuses a link.
#[cfg(not(unix))]
fn opened(path: &Path) -> std::io::Result<File> {
    File::open(path)
}

/// The refusal for `path`, which reading answered with `source`.
fn unread(path: &Path, source: std::io::Error) -> Refused {
    Refused::Unread {
        path: path.display().to_string(),
        source,
    }
}
