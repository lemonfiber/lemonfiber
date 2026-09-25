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

pub mod closure;
pub mod compose;
pub mod mounts;
pub mod standing;

use std::path::{Path, PathBuf};

use include_dir::{Dir, DirEntry};
use lemonfiber_manifest::{validate, Date, Manifest};
use thiserror::Error;

use crate::error::codes::stack::{
    STACK_INVALID, STACK_MALFORMED, STACK_NOT_EMBEDDED, STACK_NOT_SET_UP, STACK_NOT_WRITTEN,
    STACK_UNREADABLE, STACK_UNRECOGNISED, STACK_UNUSABLE,
};
use crate::error::{Diagnose, Problem, Remedy, Severity, State};

/// The manifest's filename, at the root of any stack directory.
const MANIFEST: &str = "stack.toml";

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
    /// The manifest text, however this stack is stored.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when there is no manifest, or it cannot be read.
    pub(crate) fn manifest_text(self) -> Result<String, Failure> {
        match self {
            Self::Embedded(dir) => dir
                .get_file(MANIFEST)
                .and_then(include_dir::File::contents_utf8)
                .map(ToOwned::to_owned)
                .ok_or(Failure::NotEmbedded),
            Self::External(path) => {
                let manifest = path.join(MANIFEST);
                std::fs::read_to_string(&manifest).map_err(|err| Failure::Unreadable {
                    path: manifest,
                    reason: err.to_string(),
                })
            }
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
        Manifest::from_toml(&text).map_err(refused)
    }
}

/// A manifest this build cannot read, in the terms the operator needs.
///
/// Four refusals with nothing to do with each other, and for a long time one
/// headline for all of them: an operator who had left out a quotation mark was told
/// their stack was written for a different version of lemonfiber and sent looking
/// for a build that would read it. The version headline is true of exactly two of
/// these, and the other two have answers of their own.
fn refused(err: lemonfiber_manifest::Failure) -> Failure {
    let reason = err.to_string();
    match err {
        lemonfiber_manifest::Failure::Syntax(_) => Failure::Malformed { reason },
        // Kept whole rather than joined here: the list is the point of this refusal,
        // and the rendering below is what decides how a list is shown.
        lemonfiber_manifest::Failure::Unrecognised(named) => Failure::Unrecognised {
            names: named.iter().map(ToString::to_string).collect(),
        },
        lemonfiber_manifest::Failure::UnsupportedSchema { .. }
        | lemonfiber_manifest::Failure::BinaryTooOld { .. } => Failure::Unusable { reason },
    }
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

/// The stack could not be read.
#[derive(Debug, Error)]
pub enum Failure {
    /// The named directory holds no readable manifest.
    #[error("no stack manifest at {path}: {reason}")]
    Unreadable {
        /// The manifest that was looked for, in full.
        path: PathBuf,
        /// The operating system's own words.
        reason: String,
    },
    /// The manifest was read, and this build cannot use it.
    ///
    /// The pairing, and only the pairing: a stack that declares a schema generation
    /// this build does not read, or that requires a newer binary. A file that will
    /// not parse and a name this build has never heard of are [`Failure::Malformed`]
    /// and [`Failure::Unrecognised`], because neither is answered by a version.
    #[error("the stack manifest cannot be used: {reason}")]
    Unusable {
        /// The parser's own words.
        reason: String,
    },
    /// The manifest is not TOML, so nothing in it has been read.
    #[error("the stack manifest could not be parsed: {reason}")]
    Malformed {
        /// The parser's own words, which name the line it stopped on.
        reason: String,
    },
    /// The manifest is well-formed and declares names this build does not know.
    #[error("the stack manifest declares {} names this build does not know", names.len())]
    Unrecognised {
        /// Every one of them, each naming what declared it.
        names: Vec<String>,
    },
    /// The embedded stack is not intact, which the build should have prevented.
    #[error("this build has no embedded stack manifest")]
    NotEmbedded,
    /// The embedded stack has to be written somewhere, and nowhere was named.
    #[error("no directory was named to write the stack into")]
    NowhereToWrite,
    /// The manifest parsed and contradicts itself.
    #[error("the stack manifest breaks the contract in {} places", violations.len())]
    Invalid {
        /// Every violation, each naming where it is.
        violations: Vec<String>,
    },
    /// The stack could not be written where it was asked to go.
    #[error("the stack could not be written to {path}: {reason}")]
    NotWritten {
        /// Where it was going, in full.
        path: PathBuf,
        /// The operating system's own words.
        reason: String,
    },
}

impl Diagnose for Failure {
    fn problem(&self) -> Problem {
        match self {
            Self::Unreadable { path, reason } => Problem::new(
                STACK_UNREADABLE,
                Severity::Error,
                format!("No stack was found at {}", path.display()),
                "A stack directory holds a stack.toml beside its compose files. Without one there is nothing describing what would be started.",
                Remedy::new("Point at a directory containing stack.toml")
                    .with_detail("lemonfiber --stack-dir <path>"),
            )
            .or_try(Remedy::new(
                "Drop the flag to use the stack built into lemonfiber",
            ))
            .in_state(State::Guided)
            .with_detail(reason.clone()),
            Self::Unusable { reason } => Problem::new(
                STACK_UNUSABLE,
                Severity::Error,
                "This stack was written for a different version of lemonfiber",
                "Stacks and lemonfiber are versioned separately so each can move on its own. This pairing does not line up, and guessing at the difference would fail later in a way that looks unrelated.",
                Remedy::new("Update lemonfiber, or point at a stack this version reads"),
            )
            .in_state(State::Guided)
            .with_detail(reason.clone()),
            Self::Malformed { reason } => Problem::new(
                STACK_MALFORMED,
                Severity::Error,
                "This stack file could not be read",
                "A stack.toml is written in a strict format, and this one breaks it — so nothing in the file has been read at all. The detail below is where the reader stopped, and that line is where the answer is.",
                Remedy::new("Fix the file at the line named below"),
            )
            .in_state(State::Guided)
            .with_detail(reason.clone()),
            // Every name at once, for the reason the contract faults below are given
            // at once: found by asking each declaration on its own, so the whole list
            // was knowable in one pass and learning them one run at a time is a
            // guessing game.
            Self::Unrecognised { names } => Problem::new(
                STACK_UNRECOGNISED,
                Severity::Error,
                format!("This stack declares {} names this build does not know", names.len()),
                "The file is well-formed and says things about itself in words this version has no meaning for — usually a stack from a newer lemonfiber, or a fork that has added something of its own. Starting it would quietly leave out whatever was named.",
                Remedy::new("Update lemonfiber, or change the names listed below to ones it knows"),
            )
            .in_state(State::Guided)
            .with_detail(names.join("\n")),
            // Every fault at once, because fixing them one run at a time is a
            // guessing game — and the whole list was knowable in one pass.
            Self::Invalid { violations } => Problem::new(
                STACK_INVALID,
                Severity::Error,
                format!("This stack describes {} things that cannot work", violations.len()),
                "The file is well-formed, so this is not a typo — it says things about itself that contradict each other, and starting it would fail somewhere unrelated.",
                Remedy::new("Fix the faults listed below, all of which were found in one pass"),
            )
            .in_state(State::Guided)
            .with_detail(violations.join("\n")),
            Self::NowhereToWrite => Problem::new(
                STACK_NOT_SET_UP,
                Severity::Error,
                "lemonfiber has not been set up on this machine yet",
                "The stack ships inside lemonfiber and has to be written somewhere before Docker can read it, and no location has been chosen.",
                Remedy::new("Run setup").with_detail("lemonfiber setup"),
            )
            .or_try(Remedy::new("Or operate a stack directory of your own")
                .with_detail("lemonfiber --stack-dir <path>"))
            .in_state(State::Guided),
            Self::NotWritten { path, reason } => Problem::new(
                STACK_NOT_WRITTEN,
                Severity::Error,
                format!("The stack could not be written to {}", path.display()),
                "Docker reads the stack from disk, so nothing can start until this succeeds. It is usually a permission problem or a full disk.",
                Remedy::new("Check that the location is writable and has space"),
            )
            .in_state(State::Guided)
            .with_detail(reason.clone()),
            Self::NotEmbedded => Problem::unknown(
                STACK_NOT_EMBEDDED,
                Severity::Critical,
                "This build of lemonfiber is not intact",
                "The stack that ships inside the binary is missing, which the build is supposed to make impossible.",
            ),
        }
    }
}

#[cfg(test)]
mod tests;
