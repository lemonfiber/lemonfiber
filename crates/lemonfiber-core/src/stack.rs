//! Where the stack comes from, and building a slice of it.
//!
//! The stack ships inside the binary, so the common install has no second thing
//! to fetch and nothing to go stale. An operator running their own fork points
//! at a directory instead, and everything below this module stops being able to
//! tell the difference.
//!
//! Building the Compose argument vector and running it are two responsibilities
//! that must not merge: construction is a pure function over the manifest, the
//! configuration and the environment; running it is a thin layer above
//! [`crate::ports::Runner`]. Keeping construction pure is what lets every form
//! on every platform be covered by golden files with no daemon present, and it
//! is why a rehearsal and a real run cannot disagree — they are the same
//! function.
//!
//! Form closure resolves before intersecting with the protocols the operator
//! configured, in that order: a download form resolves to both usenet and
//! torrent, then narrows to what exists, so a tunnel is never started with
//! credentials that were never supplied.
//!
//! Construction and lifecycle arrive with the compose driver. See
//! `.docs/architecture/module-layout.md`.

pub mod attached;
pub mod closure;
pub mod compose;
pub mod declared;
mod failure;
pub mod mounts;
mod remembered;
pub mod schema;
pub mod standing;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use include_dir::{Dir, DirEntry};
use lemonfiber_manifest::{validate, Date, Manifest};

use failure::refused;
pub use failure::{Failure, FAILURES};

/// The manifest's filename, at the root of any stack directory.
const MANIFEST: &str = lemonfiber_manifest::assembly::ROOT;

/// The version of lemonfiber running, which a stack's `min_cli_version` is held to.
const RUNNING: &str = env!("CARGO_PKG_VERSION");

/// One file a stack would write: its path within the stack directory, and its
/// content.
pub type StackFile = (PathBuf, &'static [u8]);

/// Where the stack lemonfiber operates is read from.
#[derive(Debug, Clone, Copy)]
pub enum Source {
    /// The stack compiled into this binary.
    ///
    /// The build refuses a stack it cannot read, so reaching this variant means
    /// the manifest already parsed once, on a machine that is not the
    /// operator's.
    Embedded(&'static Dir<'static>),
    /// A stack directory on disk, named by the operator.
    External(&'static Path),
}

impl Source {
    /// The manifest text, however this stack is stored: its root and each service's
    /// file, assembled into one.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when there is no manifest, it cannot be read, or its files
    /// break the rules about them.
    pub(crate) fn manifest_text(self) -> Result<String, Failure> {
        match self {
            Self::Embedded(dir) => {
                let root = dir
                    .get_file(MANIFEST)
                    .and_then(include_dir::File::contents_utf8)
                    .ok_or(Failure::NotEmbedded)?;
                lemonfiber_manifest::assemble(root, &embedded_services(dir)).map_err(refused)
            }
            Self::External(path) => lemonfiber_manifest::read(path).map_err(refused),
        }
    }

    /// The parsed manifest, checked against the contract.
    ///
    /// Contents are validated here rather than only when something needs them,
    /// so a stack that contradicts itself is refused before anything acts on
    /// it. `today` is passed in because one rule is about a date having not yet
    /// happened, and a validator that read the clock would accept a file on one
    /// day and refuse it on another.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the manifest cannot be read or used, and
    /// [`Failure::Invalid`] when it parses and breaks the contract.
    pub fn checked_manifest(self, today: Date) -> Result<Manifest, Failure> {
        remembered::remembered(self, today, || self.checking(today))
    }

    /// The manifest parsed and checked against the contract, afresh.
    fn checking(self, today: Date) -> Result<Manifest, Failure> {
        let manifest = self.manifest()?;
        let mut violations: Vec<String> = validate(&manifest, today)
            .iter()
            .map(ToString::to_string)
            .collect();
        // The compose files as well as the manifest, because the rule that
        // decides whether an import costs nothing or costs a second copy of every
        // file is written in the volumes rather than in `stack.toml` — and it is
        // invisible to every probe, since from the host the data root is one
        // filesystem and links work perfectly.
        //
        // For the stack lemonfiber ships and for that one only. A shipped stack
        // that splits the data root is this binary being wrong about its own
        // contents, which belongs with the manifest that will not parse: nobody
        // chose it and nobody can fix it from here. A stack directory the operator
        // pointed at is the opposite of that. The rule is ours and the stack is
        // theirs, so it is guidance there rather than a contract — refusing to
        // operate a fork over a choice that costs them disk and minutes and costs
        // lemonfiber nothing would make this tool the thing standing between an
        // operator and their own system, which is the one thing it may never be.
        // Their fork is read exactly the same way; what changes is that the answer
        // is reported as a cost they can weigh rather than raised as a refusal. See
        // [`Self::crowded_mounts`] and the storage check that carries it.
        if self.is_ours() {
            violations.extend(self.crowded_mounts().iter().map(ToString::to_string));
        }
        if violations.is_empty() {
            return Ok(manifest);
        }
        Err(Failure::Invalid { violations })
    }

    /// Every service in this stack that would see more than one mount beneath the
    /// data root, and which mounts those are.
    ///
    /// Read for both kinds of stack and refused for neither: what is done with the
    /// answer belongs to the caller, and the two callers answer differently on
    /// purpose. [`Self::checked_manifest`] turns it into a refusal for the stack
    /// lemonfiber ships, because that one breaking the rule is a broken build; the
    /// storage check turns it into a finding the operator can weigh and accept,
    /// because a fork is theirs to lay out as they like.
    ///
    /// Read afresh each time rather than remembered. An operator edits the directory
    /// they pointed lemonfiber at between one run and the next — that is what
    /// pointing at one is for — so an answer kept from last time would be about a
    /// stack that no longer exists.
    #[must_use]
    pub(crate) fn crowded_mounts(self) -> Vec<mounts::Crowded> {
        mounts::crowded(&self.compose_files())
    }

    /// Every service this stack's compose files declare, with the networks it is on.
    ///
    /// Read afresh each time, like the mounts, and for both kinds of stack: a fork's
    /// networks are the fork's, and a plugin joining it joins what the fork wrote.
    #[must_use]
    pub(crate) fn attached(
        self,
    ) -> std::collections::BTreeMap<String, std::collections::BTreeSet<String>> {
        attached::attached(&self.compose_files())
    }

    /// Every Compose file the operator's stack is run from, with its text: this stack's
    /// own and every overlay layered over it.
    ///
    /// Not the documents lemonfiber writes for installed plugins, which sit inside an
    /// operator's own stack directory: those are plugins, and the register is what says
    /// which ones are on this machine. An overlay that cannot be read contributes
    /// nothing, as a stack file that cannot be read does.
    #[must_use]
    pub(crate) fn run_from(self, overlays: &[PathBuf]) -> Vec<(PathBuf, String)> {
        let written = match self {
            Self::External(directory) => Some(directory.join(crate::plugin::OVERLAYS)),
            Self::Embedded(_) => None,
        };
        let mut files: Vec<(PathBuf, String)> = self
            .compose_files()
            .into_iter()
            .filter(|(path, _)| {
                written
                    .as_ref()
                    .is_none_or(|written| !path.starts_with(written))
            })
            .collect();
        for overlay in overlays {
            if let Ok(text) = std::fs::read_to_string(overlay) {
                files.push((overlay.clone(), text));
            }
        }
        files
    }

    /// Whether this stack is lemonfiber's own rather than the operator's.
    ///
    /// The line a rule of lemonfiber's own is either enforced or offered across.
    /// Nothing else in this module needs to tell the two apart — every other read
    /// answers for both without caring which it has.
    const fn is_ours(self) -> bool {
        matches!(self, Self::Embedded(_))
    }

    /// Every compose file in this stack, with its text.
    ///
    /// Read for both kinds of stack: an operator running their own fork is
    /// exactly who this is read for, since the shipped one is held to the mount
    /// rule by its own tests and theirs is held to it by nobody else.
    #[must_use]
    fn compose_files(self) -> Vec<(PathBuf, String)> {
        match self {
            Self::Embedded(_) => self
                .files()
                .into_iter()
                .filter(|(path, _)| is_compose(path))
                .map(|(path, text)| (path, String::from_utf8_lossy(text).into_owned()))
                .collect(),
            Self::External(directory) => on_disk(directory),
        }
    }

    /// Write the stack somewhere Compose can read it, and say where that is.
    ///
    /// Compose reads files. An embedded stack has to reach the filesystem before
    /// it can be run, and it is written out on every invocation rather than
    /// cached: the cost is a few kilobytes, and the alternative is a stale copy
    /// surviving an upgrade, which is the failure mode embedding was chosen to
    /// avoid in the first place.
    ///
    /// An external stack is already on disk and is left exactly as it is.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when there is nowhere to write to, or when writing
    /// fails.
    pub fn materialise(self, into: Option<&Path>) -> Result<PathBuf, Failure> {
        match self {
            Self::External(path) => Ok(path.to_path_buf()),
            Self::Embedded(dir) => {
                let Some(into) = into else {
                    return Err(Failure::NowhereToWrite);
                };
                let unwritable = |err: std::io::Error| Failure::NotWritten {
                    path: into.to_path_buf(),
                    reason: err.to_string(),
                };
                std::fs::create_dir_all(into).map_err(unwritable)?;
                dir.extract(into).map_err(unwritable)?;
                Ok(into.to_path_buf())
            }
        }
    }

    /// The files this stack would write, each as its path within the stack
    /// directory and its content.
    ///
    /// Empty for an external stack: it is already on disk and left exactly as it
    /// is, so there is nothing for lemonfiber to write or to compare against.
    #[must_use]
    pub fn files(self) -> Vec<StackFile> {
        let mut files = Vec::new();
        if let Self::Embedded(dir) = self {
            collect(dir, &mut files);
        }
        files
    }

    /// The parsed manifest.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the manifest cannot be read, or when this build
    /// cannot use it.
    pub fn manifest(self) -> Result<Manifest, Failure> {
        let text = self.manifest_text()?;
        let manifest = Manifest::from_toml(&text).map_err(refused)?;
        manifest.admits(RUNNING).map_err(refused)?;
        Ok(manifest)
    }
}

/// Every service file the embedded stack carries, by name, with its text.
fn embedded_services(dir: &Dir<'_>) -> BTreeMap<String, String> {
    dir.get_dir(lemonfiber_manifest::assembly::SERVICES)
        .into_iter()
        .flat_map(Dir::files)
        .filter_map(|file| {
            let name = file.path().file_name()?.to_str()?;
            let text = file.contents_utf8()?;
            Some((name.to_owned(), text.to_owned()))
        })
        .collect()
}

/// Whether a path is a file Compose would read.
fn is_compose(path: &Path) -> bool {
    path.extension()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("yml") || extension.eq_ignore_ascii_case("yaml")
        })
}

/// Every compose file under a stack directory, with its text.
///
/// A file that cannot be read contributes nothing rather than stopping the read:
/// the manifest is what says whether this is a stack at all, and it has already
/// been read by the time this runs.
fn on_disk(directory: &Path) -> Vec<(PathBuf, String)> {
    let mut files = Vec::new();
    let mut pending = vec![directory.to_path_buf()];
    while let Some(here) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&here) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            // The entry's own type, which does not follow a link. A stack
            // directory holding a link back to an ancestor would otherwise be
            // walked for ever — and this runs on every command, so it would hang
            // the whole tool over one symlink an operator is entitled to make.
            // A compose file reachable only through a link is not read, which is
            // the safe direction: this under-reports rather than never returning.
            let kind = entry.file_type();
            if kind.as_ref().is_ok_and(std::fs::FileType::is_dir) {
                pending.push(path);
            } else if kind.is_ok_and(|kind| kind.is_file()) && is_compose(&path) {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    files.push((path, text));
                }
            }
        }
    }
    files
}

/// Gather every file in an embedded directory — its path within the stack and its
/// content — recursing into subdirectories so a nested compose fragment is
/// materialised alongside the files at the root.
fn collect(dir: &'static Dir<'static>, out: &mut Vec<StackFile>) {
    for entry in dir.entries() {
        match entry {
            DirEntry::File(file) => out.push((file.path().to_path_buf(), file.contents())),
            DirEntry::Dir(sub) => collect(sub, out),
        }
    }
}

#[cfg(test)]
mod tests;
