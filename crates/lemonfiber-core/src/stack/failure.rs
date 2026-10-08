//! Why a stack could not be read, in the terms the operator needs.

use std::path::PathBuf;

use thiserror::Error;

use crate::error::codes::stack::{
    STACK_INVALID, STACK_MALFORMED, STACK_NEEDS_NEWER, STACK_NOT_EMBEDDED, STACK_NOT_SET_UP,
    STACK_NOT_WRITTEN, STACK_UNASSEMBLED, STACK_UNREADABLE, STACK_UNRECOGNISED, STACK_UNUSABLE,
};
use crate::error::{Code, Diagnose, Problem, Remedy, Severity, State};

/// Every code a stack that could not be read is refused with.
///
/// Listed so that what publishes a refusal before one is raised can name them: a read
/// built on the manifest answers one of these where it could not read it, and a client
/// told that has been told something different from an empty answer.
pub const FAILURES: [Code; 10] = [
    STACK_UNREADABLE,
    STACK_UNUSABLE,
    STACK_NEEDS_NEWER,
    STACK_MALFORMED,
    STACK_UNRECOGNISED,
    STACK_INVALID,
    STACK_UNASSEMBLED,
    STACK_NOT_SET_UP,
    STACK_NOT_WRITTEN,
    STACK_NOT_EMBEDDED,
];

/// The stack could not be read.
#[derive(Debug, Error)]
pub enum Failure {
    /// The named directory holds no readable manifest.
    #[error("no stack manifest at {path}: {reason}")]
    Unreadable {
        /// The manifest file, or the directory of service files, that was looked
        /// for, in full.
        path: PathBuf,
        /// The operating system's own words.
        reason: String,
    },
    /// The manifest was read, and this build cannot use it.
    ///
    /// A stack declaring a schema generation this build does not read. A file that
    /// will not parse, a name this build has never heard of and a stack needing a newer
    /// binary are [`Failure::Malformed`], [`Failure::Unrecognised`] and
    /// [`Failure::TooOld`], each with an answer of its own.
    #[error("the stack manifest cannot be used: {reason}")]
    Unusable {
        /// The parser's own words.
        reason: String,
    },
    /// The stack names a newer `lemonfiber` than the one running.
    #[error("the stack requires lemonfiber {required} or newer, and this is {running}")]
    TooOld {
        /// The oldest version the stack runs with, as it wrote it.
        required: String,
        /// The version running.
        running: String,
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
    /// The manifest's files are not laid out as the contract says.
    #[error("the stack manifest's files break the contract in {} places", faults.len())]
    Unassembled {
        /// Every fault, each naming the `include` entry or the file it is in.
        faults: Vec<String>,
    },
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
            Self::TooOld { required, running } => Problem::new(
                STACK_NEEDS_NEWER,
                Severity::Error,
                format!("This stack needs lemonfiber {required} or newer"),
                format!(
                    "The stack names the oldest lemonfiber it works with, and this is \
                     {running}. It relies on something this version cannot do, so nothing \
                     in it is started rather than started in part."
                ),
                Remedy::new(format!("Update lemonfiber to {required} or newer"))
                    .with_detail("lemonfiber update self"),
            )
            .or_try(Remedy::new("Or point at a stack this version runs")
                .with_detail("lemonfiber --stack-dir <path>"))
            .in_state(State::Guided),
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
            // Every fault at once, for the reason the contract faults below are.
            Self::Unassembled { faults } => Problem::new(
                STACK_UNASSEMBLED,
                Severity::Error,
                format!("This stack's manifest files break the contract in {} places", faults.len()),
                "A stack is described by stack.toml and a file per service in services/, which stack.toml's include list names. These files are not laid out that way, so which services the stack holds cannot be told.",
                Remedy::new("Fix the files named below, all of which were found in one pass"),
            )
            .in_state(State::Guided)
            .with_detail(faults.join("\n")),
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

/// A manifest this build cannot read, in the terms the operator needs.
///
/// Refusals with nothing to do with each other, and for a long time one
/// headline for all of them: an operator who had left out a quotation mark was told
/// their stack was written for a different version of lemonfiber and sent looking
/// for a build that would read it. The version headline is true of exactly two of
/// these, and the other two have answers of their own.
pub(super) fn refused(err: lemonfiber_manifest::Failure) -> Failure {
    let reason = err.to_string();
    match err {
        lemonfiber_manifest::Failure::Syntax(_) => Failure::Malformed { reason },
        // Kept whole rather than joined here: the list is the point of this refusal,
        // and the rendering below is what decides how a list is shown.
        lemonfiber_manifest::Failure::Unrecognised(named) => Failure::Unrecognised {
            names: named.iter().map(ToString::to_string).collect(),
        },
        lemonfiber_manifest::Failure::UnsupportedSchema { .. } => Failure::Unusable { reason },
        lemonfiber_manifest::Failure::BinaryTooOld { required, running } => {
            Failure::TooOld { required, running }
        }
        lemonfiber_manifest::Failure::Unreadable { path, reason } => {
            Failure::Unreadable { path, reason }
        }
        lemonfiber_manifest::Failure::Assembly(faults) => Failure::Unassembled {
            faults: faults.iter().map(ToString::to_string).collect(),
        },
    }
}
