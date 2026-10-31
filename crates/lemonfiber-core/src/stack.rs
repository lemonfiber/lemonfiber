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
mod failure;
mod remembered;
pub mod schema;
pub mod standing;

use std::path::{Path, PathBuf};

use include_dir::{Dir, DirEntry};
use lemonfiber_manifest::{validate, Date, Manifest};

use failure::refused;
pub use failure::{Failure, FAILURES};

/// The version of lemonfiber running, which a stack's `min_cli_version` is held to.
pub(crate) const RUNNING: &str = env!("CARGO_PKG_VERSION");

/// One file a stack would write: its path within the stack directory, and its
/// content.
pub type StackFile = (PathBuf, &'static [u8]);

/// A stack compiled into a binary: its manifest, whole, and the files a run writes
/// for its services, which never hold the manifest itself.
#[derive(Debug)]
pub struct Carried {
    /// The manifest, assembled.
    pub manifest: &'static str,
    /// Each service's starting configuration and every recording a claim names, as a
    /// stack directory holds them.
    pub files: &'static Dir<'static>,
}

/// The stack this build carries, as its bundle assembles it.
pub static BUNDLED: Carried = Carried {
    manifest: lemonfiber_bundled::STACK,
    files: &lemonfiber_bundled::CARRIED,
};

/// Where the stack lemonfiber operates is read from.
#[derive(Debug, Clone, Copy)]
pub enum Source {
    /// The stack compiled into this binary.
    ///
    /// The build refuses a stack it cannot read, so reaching this variant means
    /// the manifest already parsed once, on a machine that is not the
    /// operator's.
    Embedded(&'static Carried),
    /// A stack directory on disk, named by the operator.
    External(&'static Path),
}

impl Source {
    /// The manifest text, however this stack is stored: whole where it is carried, and
    /// a stack directory's root and each service's file assembled into one.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when a stack directory holds no manifest, it cannot be read,
    /// or its files break the rules about them.
    pub(crate) fn manifest_text(self) -> Result<String, Failure> {
        match self {
            Self::Embedded(carried) => Ok(carried.manifest.to_owned()),
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
        let violations: Vec<String> = validate(&manifest, today)
            .iter()
            .map(ToString::to_string)
            .collect();
        if violations.is_empty() {
            return Ok(manifest);
        }
        Err(Failure::Invalid { violations })
    }

    /// Write what a run of the stack reads beside its project, and say where that is.
    ///
    /// A carried stack's services start from files on disk, so they are written out
    /// on every invocation rather than cached: the cost is a few kilobytes, and the
    /// alternative is a stale copy surviving an upgrade. Its manifest is never
    /// written: it is read where it is carried.
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
            Self::Embedded(carried) => {
                let Some(into) = into else {
                    return Err(Failure::NowhereToWrite);
                };
                let unwritable = |err: std::io::Error| Failure::NotWritten {
                    path: into.to_path_buf(),
                    reason: err.to_string(),
                };
                std::fs::create_dir_all(into).map_err(unwritable)?;
                carried.files.extract(into).map_err(unwritable)?;
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
        if let Self::Embedded(carried) = self {
            collect(carried.files, &mut files);
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

/// Gather every file in an embedded directory — its path within the stack and its
/// content — recursing into subdirectories so a service's starting files are
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
