//! Running a program and waiting for it to finish.
//!
//! Compose is driven as a subprocess because profiles are a Compose concept and
//! reimplementing them against the Engine API would mean owning their semantics.
//! Construction of the argument vector is pure and lives elsewhere; this port
//! only executes what it is handed.

use async_trait::async_trait;
use thiserror::Error;

use crate::error::{Code, Diagnose, Problem, Remedy, Severity, State};

/// What a finished process left behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    /// The exit status, absent when a signal ended the process.
    pub status: Option<i32>,
    /// Everything written to standard output.
    pub stdout: String,
    /// Everything written to standard error.
    pub stderr: String,
}

impl Output {
    /// Whether the program exited cleanly.
    #[must_use]
    pub fn succeeded(&self) -> bool {
        self.status == Some(0)
    }
}

/// A program could not be run.
#[derive(Debug, Error)]
pub enum Failure {
    /// The program is not installed, or is not on the path.
    #[error("`{program}` was not found")]
    NotFound {
        /// The program that was looked for.
        program: String,
    },
    /// The program exists but could not be started.
    #[error("`{program}` could not be started: {reason}")]
    Unusable {
        /// The program that was found.
        program: String,
        /// The operating system's own words.
        reason: String,
    },
}

/// Raised when the program a subprocess needs is missing.
pub const MISSING_PROGRAM: Code = Code::new("PROC-1");

/// Raised when a program exists but will not start.
pub const UNUSABLE_PROGRAM: Code = Code::new("PROC-2");

impl Diagnose for Failure {
    fn problem(&self) -> Problem {
        match self {
            Self::NotFound { program } => Problem::new(
                MISSING_PROGRAM,
                Severity::Error,
                format!("{program} is not installed"),
                "lemonfiber drives your container engine through this program, so nothing can be started or stopped without it.",
                Remedy::new("Install Docker Desktop, or Docker Engine on Linux")
                    .with_detail("https://docs.docker.com/get-started/get-docker/"),
            )
            .in_state(State::Guided),
            Self::Unusable { program, reason } => Problem::new(
                UNUSABLE_PROGRAM,
                Severity::Error,
                format!("{program} is installed but would not start"),
                "The program is present, so this is usually a permission or daemon problem rather than a missing install.",
                Remedy::new("Check that the container engine is running, then try again"),
            )
            .with_detail(reason.clone()),
        }
    }
}

/// Runs a program to completion.
#[async_trait]
pub trait Runner: Send + Sync {
    /// Run `argv` and wait for it to finish.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the program cannot be run at all. A program that
    /// runs and exits non-zero is a successful call with a failed [`Output`] —
    /// those are different events and the caller decides what each means.
    async fn run(&self, argv: &[String]) -> Result<Output, Failure>;
}

#[cfg(test)]
mod tests {
    use super::{Diagnose, Failure, Output};

    #[test]
    fn only_a_zero_exit_counts_as_success() {
        let ok = Output {
            status: Some(0),
            stdout: String::new(),
            stderr: String::new(),
        };
        assert!(ok.succeeded());

        let failed = Output {
            status: Some(1),
            ..ok.clone()
        };
        assert!(!failed.succeeded());

        let signalled = Output { status: None, ..ok };
        assert!(!signalled.succeeded());
    }

    #[test]
    fn a_missing_program_sends_the_operator_somewhere_to_install_it() {
        let problem = Failure::NotFound {
            program: "docker".to_owned(),
        }
        .problem();
        assert!(problem.summary.contains("docker"));
        assert!(!problem.remedies.is_empty());
    }

    #[test]
    fn an_unusable_program_keeps_the_system_s_own_words_as_detail() {
        let problem = Failure::Unusable {
            program: "docker".to_owned(),
            reason: "permission denied".to_owned(),
        }
        .problem();
        assert_eq!(problem.detail.as_deref(), Some("permission denied"));
        assert!(!problem.remedies.is_empty());
    }

    #[test]
    fn every_failure_says_which_program_it_meant() {
        let failures = [
            Failure::NotFound {
                program: "docker".to_owned(),
            },
            Failure::Unusable {
                program: "docker".to_owned(),
                reason: "denied".to_owned(),
            },
        ];
        for failure in &failures {
            assert!(failure.to_string().contains("docker"));
            assert!(!failure.problem().remedies.is_empty());
        }
    }
}
