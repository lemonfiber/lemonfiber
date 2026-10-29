//! A throwaway copy of a service's starting files, taken off the disk when it is dropped.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::ports::random::Random;
use crate::project::entry::CONFIGURATION_TREE;

/// The most starting files one service may ship.
pub(crate) const MOST_FILES: usize = 64;

/// The most bytes one service's starting files may weigh in all.
pub(crate) const MOST_BYTES: u64 = 1024 * 1024;

/// What each throwaway directory's name begins with.
const NAMED: &str = "lemonfiber-proof-";

/// Told apart across the copies one process makes.
static COUNTED: AtomicU64 = AtomicU64::new(0);

/// Whether a directory is there, reached from a root through no link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Reached {
    /// It is there, every step to it a directory of its own.
    There,
    /// A step to it is not there.
    Absent,
}

/// Where a service's starting files are: beneath a plugin's source, under its id.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Starting<'a> {
    /// The plugin's own source.
    pub(crate) root: &'a Path,
    /// The service whose files they are.
    pub(crate) service: &'a str,
}

impl Starting<'_> {
    /// The directory they are in.
    fn at(&self) -> PathBuf {
        self.root.join(CONFIGURATION_TREE).join(self.service)
    }
}

/// The digest of a service's starting files, `sha256:<hex>` over one line per file,
/// sorted by path, each the file's SHA-256, two spaces and its path.
pub(crate) fn digest(starting: Option<Starting<'_>>) -> Result<String, String> {
    let mut lines = String::new();
    for (path, bytes) in held(starting)? {
        lines.push_str(&hex(&bytes));
        lines.push_str("  ");
        lines.push_str(&path);
        lines.push('\n');
    }
    Ok(format!("sha256:{}", hex(lines.as_bytes())))
}

/// Keep a copy of a service's starting files beneath `root`, where they are found as
/// [`Starting`] finds a plugin's own, in place of any copy kept there before.
pub(crate) fn keep(from: Starting<'_>, root: &Path) -> Result<(), String> {
    let files = held(Some(from))?;
    let into = Starting {
        root,
        service: from.service,
    }
    .at();
    let _ = std::fs::remove_dir_all(&into);
    std::fs::create_dir_all(&into)
        .and_then(|()| {
            files
                .iter()
                .try_for_each(|(path, bytes)| put(&into.join(path), bytes))
        })
        .map_err(|why| unmade(&into, &why))
}

/// `bytes` written at `at`, its directory made where it is not there.
fn put(at: &Path, bytes: &[u8]) -> std::io::Result<()> {
    at.parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(at, bytes))
}

/// The SHA-256 of `bytes`, in lower-case hexadecimal.
fn hex(bytes: &[u8]) -> String {
    crate::secret::render(ring::digest::digest(&ring::digest::SHA256, bytes).as_ref())
}

/// A copy of a service's starting files, removed when dropped.
#[derive(Debug)]
pub(crate) struct Throwaway {
    root: PathBuf,
    copy: PathBuf,
    minted: BTreeMap<String, String>,
}

impl Throwaway {
    /// A fresh copy of the files at `starting`, or an empty directory where there are none,
    /// each secret they name written as a value minted from `random` for this copy alone.
    pub(crate) fn of(starting: Option<Starting<'_>>, random: &dyn Random) -> Result<Self, String> {
        Self::beneath(&std::env::temp_dir(), starting, random)
    }

    /// A fresh copy of the files at `starting` in a directory of its own beneath `temp`.
    fn beneath(
        temp: &Path,
        starting: Option<Starting<'_>>,
        random: &dyn Random,
    ) -> Result<Self, String> {
        let minting = RefCell::new(BTreeMap::new());
        let shapes = starting
            .as_ref()
            .map(|starting| crate::plugin::seeding::shapes(starting.root))
            .unwrap_or_default();
        let value = |secret: &str| {
            let mut minted = minting.borrow_mut();
            if let Some(held) = minted.get(secret) {
                return Some(String::clone(held));
            }
            let fresh = crate::plugin::seeding::minted_as(
                random,
                crate::plugin::seeding::shape_of(&shapes, secret),
            )?;
            minted.insert(secret.to_owned(), fresh.clone());
            Some(fresh)
        };
        let files = held(starting)?
            .into_iter()
            .map(|(path, bytes)| {
                crate::plugin::seeding::written(&bytes, &value, random).map(|bytes| (path, bytes))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let minted = minting.into_inner();
        swept(temp, std::time::SystemTime::now());
        let root = temp.join(format!(
            "{NAMED}{}-{}",
            std::process::id(),
            COUNTED.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root)
            .and_then(|()| {
                let made = Self {
                    copy: root.join(CONFIGURATION_TREE),
                    root: root.clone(),
                    minted,
                };
                let filled = opened(&made.root, 0o700).and_then(|()| made.filled(&files));
                filled.map(|()| made)
            })
            .map_err(|why| unmade(&root, &why))
    }

    /// The directory the copy is in.
    pub(crate) fn path(&self) -> &Path {
        &self.copy
    }

    /// Each secret the copy's files name, with the value written for it.
    pub(crate) fn written(&self) -> impl Iterator<Item = (&str, &str)> {
        self.minted
            .iter()
            .map(|(secret, value)| (secret.as_str(), value.as_str()))
    }

    /// Every value minted for the copy, which whatever ran with it is to be scrubbed of.
    pub(crate) fn made(&self) -> impl Iterator<Item = &str> {
        self.minted.values().map(String::as_str)
    }

    /// Where `named` lies beside the copy.
    pub(crate) fn beside(&self, named: &Path) -> PathBuf {
        self.root.join(named)
    }

    /// A further empty directory beside the copy, open to whoever the service is, and
    /// removed with it.
    pub(crate) fn emptied(&self, named: &Path) -> Result<PathBuf, String> {
        let at = self.beside(named);
        std::fs::create_dir(&at)
            .and_then(|()| opened(&at, 0o777))
            .map(|()| at.clone())
            .map_err(|why| unmade(&at, &why))
    }

    /// The file `named` directly beneath the copy made readable by whoever the service runs
    /// as, which the copy's owner-only directory keeps from anybody else on the machine.
    pub(crate) fn readable(&self, named: &str) -> Result<(), String> {
        let at = self.copy.join(named);
        opened(&at, 0o644).map_err(|why| unmade(&at, &why))
    }

    /// `text` held at `named` directly beneath the copy, readable by its owner alone,
    /// written new in place of whatever was there and through no link.
    pub(crate) fn hold(&self, named: &str, text: &str) -> Result<(), String> {
        use std::io::Write as _;
        let at = self.copy.join(named);
        let _ = std::fs::remove_file(&at);
        owner_only()
            .open(&at)
            .and_then(|mut file| file.write_all(text.as_bytes()))
            .map_err(|why| unmade(&at, &why))
    }

    /// The uid and gid that own the copy, which is whoever made it.
    #[cfg(unix)]
    pub(crate) fn owner(&self) -> Option<(u32, u32)> {
        use std::os::unix::fs::MetadataExt as _;
        std::fs::metadata(&self.root)
            .ok()
            .map(|made| (made.uid(), made.gid()))
    }

    /// Nobody, where the platform names no owner.
    #[cfg(not(unix))]
    pub(crate) fn owner(&self) -> Option<(u32, u32)> {
        None
    }

    /// A file at `relative` beneath the copy, as whatever ran with it left it: read
    /// through no link, from nothing but a file beneath the copy, and none where it is not
    /// there yet.
    pub(crate) fn wrote(&self, relative: &Path) -> Result<Option<Vec<u8>>, String> {
        let named = relative.display().to_string();
        if relative.as_os_str().is_empty()
            || !relative
                .components()
                .all(|step| matches!(step, std::path::Component::Normal(_)))
        {
            return Err(format!("{named} is not a path beneath the starting files"));
        }
        let at = self.copy.join(relative);
        let parent = at.parent().unwrap_or(&self.copy);
        if linkless(&self.copy, parent, &STARTING)? == Reached::Absent {
            return Ok(None);
        }
        match std::fs::symlink_metadata(&at) {
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(why) => return Err(unread(&at, &why)),
            Ok(kind) if !kind.is_file() => return Err(unkind(&named)),
            Ok(_) => {}
        }
        let bytes = read(&at, &self.copy, &named, MOST_BYTES)?;
        if bytes.len() as u64 > MOST_BYTES {
            return Err(format!("{named} passes {MOST_BYTES} bytes"));
        }
        Ok(Some(bytes))
    }

    /// Every file written beneath the copy, every directory open to whoever the run is.
    fn filled(&self, files: &[(String, Vec<u8>)]) -> std::io::Result<()> {
        std::fs::create_dir(&self.copy)
            .and_then(|()| opened(&self.copy, 0o777))
            .and_then(|()| {
                files
                    .iter()
                    .try_for_each(|(path, bytes)| self.placed(path, bytes))
            })
    }

    /// One file written at `path` beneath the copy, with each directory on the way to it
    /// that is not there yet, all of them open to whoever the run is.
    fn placed(&self, path: &str, bytes: &[u8]) -> std::io::Result<()> {
        let at = self.copy.join(path);
        let mut parent = at.parent();
        let mut made = Vec::new();
        while let Some(dir) = parent.filter(|dir| *dir != self.copy && !dir.exists()) {
            made.push(dir.to_path_buf());
            parent = dir.parent();
        }
        made.iter()
            .rev()
            .try_for_each(|dir| std::fs::create_dir(dir).and_then(|()| opened(dir, 0o777)))
            .and_then(|()| std::fs::write(&at, bytes))
            .and_then(|()| opened(&at, 0o666))
    }
}

/// Remove every copy beneath `temp` that a run before this one left there.
pub(super) fn swept(temp: &Path, now: std::time::SystemTime) {
    let Ok(entries) = std::fs::read_dir(temp) else {
        return;
    };
    for at in entries.flatten().map(|entry| entry.path()) {
        let ours = at
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with(NAMED));
        let old = std::fs::symlink_metadata(&at)
            .ok()
            .filter(std::fs::Metadata::is_dir)
            .and_then(|kind| kind.modified().ok())
            .and_then(|modified| now.duration_since(modified).ok())
            .is_some_and(|age| age > crate::ports::program::LEFT_BEHIND);
        if ours && old {
            let _ = std::fs::remove_dir_all(&at);
        }
    }
}

impl Drop for Throwaway {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Why a copy could not be made.
fn unmade(at: &Path, why: &std::io::Error) -> String {
    format!(
        "the throwaway copy of the starting files could not be made at {}: {why}",
        at.display()
    )
}

/// A file created new, never through a link, readable and writable by its owner alone.
#[cfg(unix)]
fn owner_only() -> std::fs::OpenOptions {
    use std::os::unix::fs::OpenOptionsExt as _;
    let mut options = std::fs::File::options();
    options.write(true).create_new(true).mode(0o600);
    options
}

/// A file created new, where the platform keeps no mode.
#[cfg(not(unix))]
fn owner_only() -> std::fs::OpenOptions {
    let mut options = std::fs::File::options();
    options.write(true).create_new(true);
    options
}

/// `at` given `mode`, where the platform keeps one.
#[cfg(unix)]
fn opened(at: &Path, mode: u32) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(at, std::fs::Permissions::from_mode(mode))
}

/// Nothing, where the platform keeps no mode.
#[cfg(not(unix))]
fn opened(_at: &Path, _mode: u32) -> std::io::Result<()> {
    Ok(())
}

/// Every file of a service's starting files, with its path beneath their directory,
/// sorted; none where the directory is not there, and refused where a link stands
/// anywhere from the source down, where anything is neither a file nor a directory, or
/// where they pass the bounds.
pub(crate) fn held(starting: Option<Starting<'_>>) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut found = Vec::new();
    let Some(starting) = starting else {
        return Ok(found);
    };
    let top = starting.at();
    if linkless(starting.root, &top, &STARTING)? == Reached::Absent {
        return Ok(found);
    }
    let mut weight = 0_u64;
    let mut waiting = vec![top.clone()];
    while let Some(dir) = waiting.pop() {
        for (at, kind) in listed(&dir).map_err(|why| unread(&dir, &why))? {
            let named = at.strip_prefix(&top).unwrap_or(&at).display().to_string();
            if kind.is_dir() {
                waiting.push(at);
                continue;
            }
            if !kind.is_file() {
                return Err(unkind(&named));
            }
            let bytes = read(&at, &top, &named, MOST_BYTES.saturating_sub(weight))?;
            weight = weight.saturating_add(bytes.len() as u64);
            found.push((named, bytes));
            if found.len() > MOST_FILES || weight > MOST_BYTES {
                return Err(format!(
                    "the starting files pass {MOST_FILES} files or {MOST_BYTES} bytes"
                ));
            }
        }
    }
    found.sort();
    Ok(found)
}

/// Every entry of `dir`, with what it is at its own name, never through a link.
fn listed(dir: &Path) -> std::io::Result<Vec<(PathBuf, std::fs::Metadata)>> {
    std::fs::read_dir(dir).and_then(|entries| {
        entries
            .map(|entry| {
                entry.and_then(|entry| {
                    let at = entry.path();
                    std::fs::symlink_metadata(&at).map(|kind| (at, kind))
                })
            })
            .collect()
    })
}

/// Refuse where a directory a container is handed does not lie where throwaway copies are
/// made, or is reached from there through a link.
pub(crate) fn unlinked<'a>(sources: impl IntoIterator<Item = &'a Path>) -> Result<(), String> {
    let temp = std::env::temp_dir();
    sources.into_iter().try_for_each(|source| {
        if !source.starts_with(&temp) {
            return Err(format!(
                "{} does not lie where lemonfiber makes its throwaway copies, so no container \
                 was started",
                source.display()
            ));
        }
        linkless(&temp, source, &MOUNTED).map(drop)
    })
}

/// Whether the directory `top` is there, every step to it from `root` a directory of its
/// own; refused in `refusals`' words where one is a link or anything else.
pub(super) fn linkless(root: &Path, top: &Path, refusals: &Refusals) -> Result<Reached, String> {
    let mut here = root.to_path_buf();
    for step in top.strip_prefix(root).unwrap_or(top).components() {
        here.push(step);
        match std::fs::symlink_metadata(&here) {
            Ok(kind) if kind.is_dir() => {}
            Ok(_) => return Err((refusals.unkind)(&here.display().to_string())),
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => return Ok(Reached::Absent),
            Err(why) => return Err((refusals.unread)(&here, &why)),
        }
    }
    Ok(Reached::There)
}

/// The refusal of something in the starting files that is not a file or a directory.
fn unkind(named: &str) -> String {
    format!("{named} is a link or a device, and no starting file may be either")
}

/// The refusal of a container handed a directory reached through something that is not a
/// directory.
fn unmounted(named: &str) -> String {
    format!("{named} is a link or a device, so no container was started")
}

/// The refusal of a container handed a directory that could not be looked at.
fn unlooked(at: &Path, why: &std::io::Error) -> String {
    format!(
        "{} could not be looked at, so no container was started: {why}",
        at.display()
    )
}

/// What a walk from a root says where a step is not a directory or cannot be looked at.
pub(super) struct Refusals {
    /// Where a step is a link or anything else but a directory.
    pub(super) unkind: fn(&str) -> String,
    /// Where a step cannot be looked at.
    pub(super) unread: fn(&Path, &std::io::Error) -> String,
}

/// A walk through the starting files.
const STARTING: Refusals = Refusals { unkind, unread };

/// A walk to a directory a container is handed.
const MOUNTED: Refusals = Refusals {
    unkind: unmounted,
    unread: unlooked,
};

/// One file, opened through no link at its name and never waited on, held by the handle
/// that was opened to being a file, the one its name resolves to beneath `top`, and
/// read to at most one byte past `room`.
fn read(at: &Path, top: &Path, named: &str, room: u64) -> Result<Vec<u8>, String> {
    use std::io::Read as _;
    let read = unfollowed(at).and_then(|file| {
        let kinds = file.metadata().and_then(|kind| {
            std::fs::canonicalize(top).and_then(|inside| {
                std::fs::canonicalize(at).and_then(|resolved| {
                    std::fs::metadata(&resolved).map(|named_kind| {
                        kind.is_file() && resolved.starts_with(inside) && same(&kind, &named_kind)
                    })
                })
            })
        });
        kinds.and_then(|plain| {
            let mut bytes = Vec::new();
            if plain {
                file.take(room.saturating_add(1))
                    .read_to_end(&mut bytes)
                    .map(|_| bytes)
            } else {
                Err(std::io::ErrorKind::InvalidInput.into())
            }
        })
    });
    read.map_err(|why| {
        if why.kind() == std::io::ErrorKind::InvalidInput {
            unkind(named)
        } else {
            unread(at, &why)
        }
    })
}

/// Whether two readings are of the one file.
#[cfg(unix)]
fn same(one: &std::fs::Metadata, other: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    one.dev() == other.dev() && one.ino() == other.ino()
}

/// Whether two readings are of the one file, where the platform names none.
#[cfg(not(unix))]
fn same(one: &std::fs::Metadata, other: &std::fs::Metadata) -> bool {
    one.len() == other.len()
}

/// `at`, opened for reading, refusing a link at its name and refusing to wait.
#[cfg(unix)]
fn unfollowed(at: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt as _;
    std::fs::File::options()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(at)
}

/// `at`, opened for reading, where the platform has no flag refusing a link.
#[cfg(not(unix))]
fn unfollowed(at: &Path) -> std::io::Result<std::fs::File> {
    std::fs::File::open(at)
}

/// Why the starting files could not be read.
fn unread(at: &Path, why: &std::io::Error) -> String {
    format!(
        "the starting files could not be read at {}: {why}",
        at.display()
    )
}

#[cfg(test)]
mod tests;
