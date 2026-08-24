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

use crate::error::{Code, Diagnose, Problem, Remedy, Severity, State};

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
    pub fn manifest_text(self) -> Result<String, Failure> {
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
        violations.extend(
            mounts::crowded(&self.compose_files())
                .iter()
                .map(ToString::to_string),
        );
        if violations.is_empty() {
            return Ok(manifest);
        }
        Err(Failure::Invalid { violations })
    }

    /// Every compose file in this stack, with its text.
    ///
    /// Read for both kinds of stack: an operator running their own fork is
    /// exactly who this protects, since the shipped one is held to the rule by
    /// its own tests.
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
        Manifest::from_toml(&text).map_err(|err| Failure::Unusable {
            reason: err.to_string(),
        })
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
    #[error("the stack manifest cannot be used: {reason}")]
    Unusable {
        /// The parser's own words.
        reason: String,
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

/// Raised when a stack directory holds no readable manifest.
pub const STACK_UNREADABLE: Code = Code::new("STACK-1");

/// Raised when a manifest is readable and this build cannot use it.
pub const STACK_UNUSABLE: Code = Code::new("STACK-2");

/// Raised when the embedded stack is not intact.
pub const STACK_NOT_EMBEDDED: Code = Code::new("STACK-3");

/// Raised when a manifest parses and breaks the contract.
pub const STACK_INVALID: Code = Code::new("STACK-6");

/// Raised when lemonfiber has nowhere to write the stack.
pub const STACK_NOT_SET_UP: Code = Code::new("STACK-4");

/// Raised when the stack could not be written to disk.
pub const STACK_NOT_WRITTEN: Code = Code::new("STACK-5");

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
mod tests {
    use std::path::Path;

    use include_dir::{include_dir, Dir};

    use super::{Diagnose, Failure, Source};
    use crate::error::{Severity, State};

    /// The same stack the binary embeds, so both variants are exercised against
    /// the real thing rather than against a fixture that could drift from it.
    static EMBEDDED: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../assets/media-stack");

    /// A directory that is certainly not a stack.
    ///
    /// `adapters` because the crate cannot compile without it. The previous choice was a
    /// directory that later moved out to its own crate, and an emptied directory fails
    /// here rather than where it was emptied.
    static NOT_A_STACK: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/src/adapters");

    /// The stack this repository carries as a submodule, read from disk.
    fn checked_out() -> Source {
        Source::External(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/media-stack"
        )))
    }

    #[test]
    fn reads_a_manifest_compiled_into_the_binary() {
        let read = Source::Embedded(&EMBEDDED)
            .manifest()
            .ok()
            .map(|manifest| (manifest.schema_version, manifest.services.len()));
        assert_eq!(read, Some((1, 19)));
    }

    #[test]
    fn the_embedded_and_external_readings_agree() {
        assert_eq!(
            Source::Embedded(&EMBEDDED).manifest_text().ok(),
            checked_out().manifest_text().ok(),
            "the same stack read two ways is the same stack"
        );
    }

    #[test]
    fn an_embedded_directory_with_no_manifest_is_a_broken_build() {
        let refusal = Source::Embedded(&NOT_A_STACK).manifest().err();
        assert!(matches!(refusal, Some(Failure::NotEmbedded)));
    }

    #[test]
    fn reads_a_manifest_from_a_directory() {
        let read = checked_out()
            .manifest()
            .ok()
            .map(|manifest| (manifest.schema_version, manifest.services.len()));
        assert_eq!(read, Some((1, 19)));
    }

    #[test]
    fn a_readable_manifest_from_another_generation_is_refused_as_such() {
        let future = Source::External(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/future-schema"
        )));
        let refusal = future.manifest().err().map(|err| err.to_string());
        assert_eq!(
            refusal.as_deref(),
            Some("the stack manifest cannot be used: the manifest declares schema version 99, and this build reads [1]"),
            "read cleanly, and refused on the pairing rather than the syntax"
        );
    }

    #[test]
    fn a_directory_with_no_manifest_shows_the_path_it_looked_for() {
        let missing = Source::External(Path::new("/lemonfiber/no/such/stack"));
        let refusal = missing.manifest().err().map(|err| err.to_string());
        assert_eq!(
            refusal
                .as_deref()
                .map(|message| message.contains("/lemonfiber/no/such/stack/stack.toml")),
            Some(true),
            "the path is shown in full: {refusal:?}"
        );
    }

    /// A directory of our own under the system temporary directory.
    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("lemonfiber-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn an_embedded_stack_is_written_where_compose_can_read_it() {
        let dir = scratch("materialise");
        let written = Source::Embedded(&EMBEDDED).materialise(Some(&dir));

        assert_eq!(written.ok().as_deref(), Some(dir.as_path()));
        assert!(dir.join("stack.toml").is_file(), "the manifest is written");
        assert!(
            dir.join("compose.yml").is_file(),
            "so are the compose files"
        );
        assert!(
            dir.join("compose").join("tv.yml").is_file(),
            "including the fragments, which the root file includes by path"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn writing_the_stack_twice_is_the_same_as_writing_it_once() {
        let dir = scratch("materialise-twice");
        let first = Source::Embedded(&EMBEDDED).materialise(Some(&dir)).is_ok();
        let second = Source::Embedded(&EMBEDDED).materialise(Some(&dir)).is_ok();
        assert!(first && second, "an existing directory is written over");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_external_stack_is_left_exactly_where_it_is() {
        let path = Path::new("/opt/somebody/stack");
        assert_eq!(
            Source::External(path).materialise(None).ok().as_deref(),
            Some(path),
            "nothing is written, and no destination is needed"
        );
    }

    #[test]
    fn an_embedded_stack_with_nowhere_to_go_says_setup_has_not_run() {
        let refusal = Source::Embedded(&EMBEDDED).materialise(None).err();
        assert!(matches!(refusal, Some(Failure::NowhereToWrite)));

        let problem = Failure::NowhereToWrite.problem();
        assert_eq!(
            problem.remedies.len(),
            2,
            "run setup, or bring your own stack"
        );
    }

    #[test]
    fn a_destination_that_cannot_be_written_names_it() {
        // A file where a directory needs to be: the closest thing to a
        // permission failure that behaves the same way on every platform.
        let blocker = scratch("not-a-directory");
        if let Some(parent) = blocker.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&blocker, "in the way");

        let refusal = Source::Embedded(&EMBEDDED)
            .materialise(Some(&blocker.join("stack")))
            .err()
            .map(|err| err.to_string());
        assert_eq!(
            refusal
                .as_deref()
                .map(|m| m.contains("could not be written")),
            Some(true),
            "got: {refusal:?}"
        );

        let _ = std::fs::remove_file(&blocker);
    }

    #[test]
    fn a_missing_stack_offers_both_ways_out() {
        let problem = Failure::Unreadable {
            path: "/tmp/nowhere/stack.toml".into(),
            reason: "No such file or directory".to_owned(),
        }
        .problem();
        assert_eq!(problem.state, State::Guided);
        assert_eq!(
            problem.remedies.len(),
            2,
            "point somewhere else, or stop pointing"
        );
        assert!(problem.summary.contains("/tmp/nowhere/stack.toml"));
    }

    #[test]
    fn an_unusable_manifest_reads_as_a_pairing_rather_than_a_syntax_error() {
        let reason = "the manifest declares schema version 99, and this build reads [1]";
        let problem = Failure::Unusable {
            reason: reason.to_owned(),
        }
        .problem();
        assert!(problem.summary.contains("different version"));
        assert_eq!(problem.detail.as_deref(), Some(reason));
    }

    #[test]
    fn a_build_that_lost_its_stack_admits_it_rather_than_guessing() {
        let problem = Failure::NotEmbedded.problem();
        assert_eq!(problem.severity, Severity::Critical);
        assert_eq!(problem.state, State::Unknown);
        assert!(!problem.remedies.is_empty(), "escalation is still offered");
    }

    #[test]
    fn every_failure_says_something_and_offers_something() {
        let failures = [
            Failure::Unreadable {
                path: "/tmp/x/stack.toml".into(),
                reason: "denied".to_owned(),
            },
            Failure::Unusable {
                reason: "schema 99".to_owned(),
            },
            Failure::NotEmbedded,
            Failure::NowhereToWrite,
            Failure::NotWritten {
                path: "/tmp/x".into(),
                reason: "denied".to_owned(),
            },
        ];
        for failure in &failures {
            assert!(!failure.to_string().is_empty());
            assert!(!failure.problem().remedies.is_empty());
        }
    }

    /// Today, for the checks that take a date. Far enough forward that nothing in
    /// the shipped stack has aged out of its own freshness rule.
    const fn today() -> lemonfiber_manifest::Date {
        lemonfiber_manifest::Date {
            year: 2026,
            month: 8,
            day: 14,
        }
    }

    /// What checking a stack complained about, in the words the operator is
    /// shown — empty where it complained about nothing.
    fn refusal(source: Source) -> String {
        source
            .checked_manifest(today())
            .err()
            .map(|failure| failure.problem())
            .and_then(|problem| problem.detail)
            .unwrap_or_default()
    }

    #[test]
    fn the_stack_we_ship_obeys_its_own_single_mount_rule() {
        // The rule is invisible to every probe — from the host the data root is
        // one filesystem and links work perfectly — so the only thing that can
        // hold the shipped stack to it is this.
        assert_eq!(refusal(Source::Embedded(&EMBEDDED)), "");
    }

    #[test]
    fn a_stack_that_splits_the_data_root_is_refused() {
        // The failure this exists for: two mounts beneath the data root put the
        // download and the library on opposite sides of a filesystem boundary
        // inside the container, and every import silently becomes a copy.
        let dir = std::env::temp_dir().join(format!("lemonfiber-split-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(std::fs::create_dir_all(&dir).is_ok());
        assert!(std::fs::copy(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/media-stack/stack.toml"
            ),
            dir.join("stack.toml")
        )
        .is_ok());
        // A raw literal on its own lines: a continued string here is reflowed by
        // the formatter into one indented line, which YAML then reads as a
        // continuation of the previous entry rather than a second mount — a
        // fixture that quietly stops describing the thing it is testing.
        let split = r"services:
  sonarr:
    volumes:
      - ${DATA_ROOT}/downloads:/downloads
      - ${DATA_ROOT}/media:/media
";
        assert!(std::fs::write(dir.join("split.yml"), split).is_ok());

        // Leaked deliberately: `Source::External` holds a `&'static Path`, and a
        // stack directory outliving one test is a few kilobytes in a scratch dir.
        let path: &'static Path = Box::leak(dir.clone().into_boxed_path());
        // Asserted on what the operator is actually shown, rather than on the
        // shape of the error behind it.
        let said = refusal(Source::External(path));
        assert!(said.contains("sonarr"), "{said}");
        assert!(said.contains("copied rather than hardlinked"), "{said}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_link_back_to_an_ancestor_is_not_walked_into() {
        // A stack directory is the operator's own, and a link inside it is theirs
        // to make. Following one back to an ancestor would walk for ever, on every
        // command — so the entry's own type decides, and a link is not a directory.
        let dir = std::env::temp_dir().join(format!("lemonfiber-loop-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let inner = dir.join("compose");
        assert!(std::fs::create_dir_all(&inner).is_ok());
        assert!(std::fs::write(inner.join("tv.yml"), "services: {}\n").is_ok());
        // Where the platform makes one cheaply. Its absence changes no branch: the
        // walk reads the entry's type either way.
        #[cfg(unix)]
        let _ = std::os::unix::fs::symlink(&dir, inner.join("back"));

        let read = super::on_disk(&dir);
        assert_eq!(read.len(), 1, "the real file, once: {read:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_directory_that_cannot_be_read_contributes_nothing_rather_than_stopping() {
        // Whether this is a stack at all is the manifest's business, and it has
        // already been read by the time this runs. A path that is not a directory
        // to walk simply has no compose files in it.
        assert!(super::on_disk(Path::new("/lemonfiber-no-such-directory")).is_empty());
    }
}
