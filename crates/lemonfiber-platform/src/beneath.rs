//! Making, writing, reading, moving and removing beneath a directory somebody else can
//! write, never through a link.
//!
//! On Unix every step from the directory down is opened through the handle of the one
//! above it, refusing a link and refusing to wait, so a link put on the way between one
//! step and the next is refused rather than followed. Windows has no flag that refuses a
//! link on the way, so each step there is looked at first and refused where it is one.
//!
//! What a caller means by a refusal is its own: every operation here answers with a
//! [`Refusal`], which is either the one refusal of something that is not a plain file of
//! the directory's own, or the platform's words for anything else.

use std::ffi::{OsStr, OsString};
use std::path::{Component, Path};

use crate::fs::{OWNER_DIRECTORY, OWNER_FILE, SHARED_DIRECTORY, SHARED_FILE};

/// Why an operation beneath a directory was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The path is a link, leads outside the directory, or is not a plain file.
    Escaped,
    /// Anything else, in words.
    Said(String),
}

/// Where a path lies beneath a directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lies {
    /// Beneath it, reached through no link.
    Plain,
    /// Nothing is there.
    Absent,
    /// A link stands on the way, or the path is not beneath it at all.
    Escaped,
}

/// What a file and the directories above it are made with, and whose they must be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kept {
    /// The mode a file is made with and held to.
    pub file: u32,
    /// The mode a directory is made with.
    pub directory: u32,
    /// Whether every directory on the way must be the writer's own.
    pub owned: bool,
}

impl Kept {
    /// Readable by its owner alone, in directories of the owner's own.
    pub const OWNER: Self = Self {
        file: OWNER_FILE,
        directory: OWNER_DIRECTORY,
        owned: true,
    };

    /// Readable by its owner alone, in directories of anybody's.
    pub const PRIVATE: Self = Self {
        file: OWNER_FILE,
        directory: OWNER_DIRECTORY,
        owned: false,
    };

    /// Readable by anybody.
    pub const SHARED: Self = Self {
        file: SHARED_FILE,
        directory: SHARED_DIRECTORY,
        owned: false,
    };
}

/// The names `path` is made of below `within`, refused where it is not beneath it or
/// steps back up out of it.
///
/// # Errors
///
/// Where `path` does not lie beneath `within` by its names alone.
pub fn steps<'a>(path: &'a Path, within: &Path) -> Result<Vec<&'a OsStr>, Refusal> {
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
fn outside(path: &Path, within: &Path) -> Refusal {
    Refusal::Said(format!(
        "{} does not lie beneath {}, and lemonfiber makes its own directories only there",
        path.display(),
        within.display()
    ))
}

/// The refusal of moving `from` to `to`, saying `why`.
fn unmoved(from: &Path, to: &Path, why: &str) -> Refusal {
    Refusal::Said(format!(
        "{} was not moved to {}: {why}, and lemonfiber never moves through a link or over \
         something already there",
        from.display(),
        to.display()
    ))
}

/// The name a file is written under beside `name` before it is renamed over it.
fn staging(name: &OsStr) -> OsString {
    let mut staging = OsString::from(".");
    staging.push(name);
    staging.push(".writing");
    staging
}

/// The refusal of writing `path`.
fn unwritten(path: &Path, why: &str) -> Refusal {
    Refusal::Said(format!("{} was not written: {why}", path.display()))
}

/// The refusal of taking `path` away.
fn unremoved(path: &Path, why: &str) -> Refusal {
    Refusal::Said(format!("{} was not taken away: {why}", path.display()))
}

/// The refusal of reading `path`.
fn unread(path: &Path, why: &str) -> Refusal {
    Refusal::Said(format!("{} was not read: {why}", path.display()))
}

/// The refusal of writing what `names` lead to beneath `root`, saying `why`.
fn unpacked(root: &Path, names: &[&OsStr], why: &str) -> Refusal {
    let path = names
        .iter()
        .fold(root.to_path_buf(), |at, name| at.join(name));
    Refusal::Said(format!("{} was not written: {why}", path.display()))
}

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::*;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

#[cfg(test)]
mod tests;
