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

use std::path::{Path, PathBuf};

use include_dir::Dir;
use lemonfiber_manifest::Manifest;
use thiserror::Error;

use crate::error::{Code, Diagnose, Problem, Remedy, Severity, State};

/// The manifest's filename, at the root of any stack directory.
const MANIFEST: &str = "stack.toml";

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
}

/// Raised when a stack directory holds no readable manifest.
pub const STACK_UNREADABLE: Code = Code::new("STACK-1");

/// Raised when a manifest is readable and this build cannot use it.
pub const STACK_UNUSABLE: Code = Code::new("STACK-2");

/// Raised when the embedded stack is not intact.
pub const STACK_NOT_EMBEDDED: Code = Code::new("STACK-3");

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
    static NOT_A_STACK: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/src/ports");

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
        ];
        for failure in &failures {
            assert!(!failure.to_string().is_empty());
            assert!(!failure.problem().remedies.is_empty());
        }
    }
}
