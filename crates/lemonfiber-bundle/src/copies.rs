//! The copies beside the bundle, read without following a link.

use std::collections::BTreeMap;
use std::io::ErrorKind;
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
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(missing) if missing.kind() == ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(source) => return Err(unread(directory, source)),
    };
    let mut copies = BTreeMap::new();
    for entry in entries {
        let entry = entry.map_err(|source| unread(directory, source))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == KEEP {
            continue;
        }
        let path = entry.path();
        let kind = entry.file_type().map_err(|source| unread(&path, source))?;
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
fn walked(root: &Path) -> Result<Copy, Refused> {
    let mut copy = Copy::default();
    let mut pending = vec![String::new()];
    while let Some(relative) = pending.pop() {
        let at = if relative.is_empty() {
            root.to_path_buf()
        } else {
            root.join(&relative)
        };
        let entries = std::fs::read_dir(&at).map_err(|source| unread(&at, source))?;
        for entry in entries {
            let entry = entry.map_err(|source| unread(&at, source))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = if relative.is_empty() {
                name
            } else {
                format!("{relative}/{name}")
            };
            let kind = entry
                .file_type()
                .map_err(|source| unread(&entry.path(), source))?;
            if kind.is_symlink() {
                copy.links.push(path);
            } else if kind.is_dir() {
                pending.push(path);
            } else if path == MANIFEST {
                copy.manifest = Some(text(&entry.path())?);
            } else {
                let bytes =
                    std::fs::read(entry.path()).map_err(|source| unread(&entry.path(), source))?;
                copy.files.insert(path, bytes);
            }
        }
    }
    copy.links.sort();
    Ok(copy)
}

/// The text at `path`.
fn text(path: &Path) -> Result<String, Refused> {
    std::fs::read_to_string(path).map_err(|source| unread(path, source))
}

/// The refusal for `path`, which reading answered with `source`.
fn unread(path: &Path, source: std::io::Error) -> Refused {
    Refused::Unread {
        path: path.display().to_string(),
        source,
    }
}
