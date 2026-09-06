//! Handing a long-running command to the machine's own service manager.
//!
//! A command that guards a volume or closes requests on a clock has to outlive
//! the terminal it was typed in, and the only thing on any of these platforms
//! that can promise that is the service manager the operating system ships. What
//! crosses this seam is one command described in the terms every manager needs —
//! a program, its arguments, and somewhere to put the words it would otherwise
//! have said on a terminal — and never a plist, a unit or a `launchctl`
//! invocation, because those are three spellings of one idea and only the
//! adapter should know which spelling it is holding.
//!
//! The name that crosses it is lemonfiber's own — `watch`, `expiring` — rather
//! than the manager's. A launch agent is conventionally named in reverse
//! domain order and a systemd unit is not, so a caller passing a label would be
//! a caller deciding which manager it was talking to.
//!
//! Nothing here reports success from having written a file. [`Standing`] is what
//! the manager says, and it has a word for the manager that will not say.
//!
//! See `.docs/architecture/ports-and-adapters.md`.

use std::path::PathBuf;

use async_trait::async_trait;
use serde::Serialize;
use thiserror::Error;

use crate::error::{Code, Diagnose, Problem, Remedy, Severity, State};

/// The service manager a machine has, or the absence of one lemonfiber configures.
///
/// The absence is the default, because a machine nobody has told is a machine
/// nothing is known about, and guessing at a manager is how a report comes to
/// claim a platform it never asked.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Manager {
    /// macOS, through a launch agent in the operator's own login session.
    Launchd,
    /// Linux, through a user service in the operator's own session.
    Systemd,
    /// A platform lemonfiber does not configure.
    #[default]
    Unsupported,
}

impl Manager {
    /// What this manager is called, in the words its own documentation uses.
    #[must_use]
    pub const fn named(self) -> &'static str {
        match self {
            Self::Launchd => "launchd",
            Self::Systemd => "systemd",
            Self::Unsupported => "none",
        }
    }

    /// Whether lemonfiber can install anything here.
    #[must_use]
    pub const fn configurable(self) -> bool {
        !matches!(self, Self::Unsupported)
    }
}

/// One command to be kept running, described the way every manager needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hosted {
    /// lemonfiber's own name for it, which the adapter turns into a label.
    pub name: String,
    /// The program to run, which is this binary.
    pub program: PathBuf,
    /// The arguments to run it with — the command the operator would have typed.
    pub arguments: Vec<String>,
    /// Where its words go, since a hosted command has no terminal to say them in.
    pub output: PathBuf,
    /// One sentence saying what it is, for a manager that carries a description.
    pub about: String,
}

/// What the manager says about a name it may or may not hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// Nothing is installed under this name.
    Absent,
    /// Installed, and the manager says it is running.
    Running,
    /// Installed, and the manager says it is not running.
    Stopped,
    /// Installed, and the manager would not say either way.
    Unsaid,
}

/// The program an installed definition names, and whether it is still there.
///
/// The two are one value because they are read together and mean nothing apart:
/// a path with no answer about whether it exists would have every caller asking
/// the filesystem a second time, and an existence with no path would have none of
/// them able to say what is missing. lemonfiber updating itself is the ordinary
/// way the second becomes false.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    /// Where the definition says the program is.
    pub at: PathBuf,
    /// Whether anything is there now.
    pub present: bool,
}

/// What the manager holds under one name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    /// What the manager says about it.
    pub standing: Standing,
    /// The definition the manager reads, where one is installed.
    pub definition: Option<PathBuf>,
    /// The program that definition names.
    pub program: Option<Program>,
    /// The whole command line that definition runs, as one string to show.
    pub runs: Option<String>,
    /// Where that definition writes the command's words.
    pub output: Option<PathBuf>,
}

impl Held {
    /// Nothing installed under this name.
    #[must_use]
    pub const fn absent() -> Self {
        Self {
            standing: Standing::Absent,
            definition: None,
            program: None,
            runs: None,
            output: None,
        }
    }

    /// Whether this is installed against a program that is no longer there.
    #[must_use]
    pub fn orphaned(&self) -> bool {
        self.program
            .as_ref()
            .is_some_and(|program| !program.present)
    }
}

/// What an install left on the machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placed {
    /// The definition file the manager now reads.
    pub definition: PathBuf,
    /// Whether installing it also started it.
    pub started: bool,
}

/// A service manager would not do as it was asked.
///
/// Comparable, unlike most of the failures at this boundary, and for a reason
/// this one has and they do not: what a caller does about it turns on which of
/// the three it is, so a test asserting the right one came back has to be able to
/// say so. Nothing here holds a socket or a handle, so the comparison is over
/// words the platform gave us and nothing else.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum Failure {
    /// This platform has no service manager lemonfiber configures.
    #[error("this platform has no service manager lemonfiber configures")]
    Unhostable,
    /// The definition could not be written where the manager reads them.
    #[error("the service definition could not be written to {at}: {reason}")]
    Unwritable {
        /// Where it was going.
        at: PathBuf,
        /// The operating system's own words.
        reason: String,
    },
    /// The manager was reached and refused.
    #[error("{manager} would not do it: {reason}")]
    Refused {
        /// The manager that refused.
        manager: &'static str,
        /// Its own words.
        reason: String,
    },
}

/// Raised where the platform has no service manager lemonfiber can configure.
pub const NOTHING_TO_HOST_WITH: Code = Code::new("HOST-1");

/// Raised where a service definition could not be written.
pub const DEFINITION_UNWRITABLE: Code = Code::new("HOST-2");

/// Raised where the service manager refused what it was asked.
pub const MANAGER_REFUSED: Code = Code::new("HOST-3");

impl Diagnose for Failure {
    fn problem(&self) -> Problem {
        match self {
            Self::Unhostable => Problem::new(
                NOTHING_TO_HOST_WITH,
                Severity::Warning,
                "this machine has no service manager lemonfiber can configure",
                "The command can still be run, and it will still stop when the terminal running it closes.",
                Remedy::new(
                    "Keep the command running yourself, or arrange it with whatever this system uses to start things at login",
                ),
            )
            .in_state(State::Guided),
            Self::Unwritable { at, reason } => Problem::new(
                DEFINITION_UNWRITABLE,
                Severity::Error,
                format!("the service could not be written to {}", at.display()),
                "Nothing was installed, so nothing is running and nothing was left behind.",
                Remedy::new("Check that the directory exists and belongs to you, then try again"),
            )
            .with_detail(reason.clone()),
            Self::Refused { manager, reason } => Problem::new(
                MANAGER_REFUSED,
                Severity::Error,
                format!("{manager} would not take the service"),
                "The definition that had been written was removed again, so nothing is half-installed.",
                Remedy::new("Read what it said below, then try again once that is dealt with"),
            )
            .with_detail(reason.clone()),
        }
    }
}

/// Installing, reading and removing a long-running command as a service.
///
/// Four operations rather than a file interface, because what a caller wants is
/// never "write this file" — it is "keep this running", "is it running", and
/// "take it back off". A wider seam would have every implementation of it
/// carrying methods nothing calls.
#[async_trait]
pub trait Host: Send + Sync {
    /// Which service manager this machine has.
    ///
    /// Synchronous and infallible: it is a property of the platform rather than
    /// something asked of it, and a caller that could not find out would have to
    /// decide twice about one thing.
    fn manager(&self) -> Manager;

    /// Install it, replacing anything already installed under the same name.
    ///
    /// Replacing rather than refusing, because two services closing the same
    /// requests is the outcome nothing should be able to reach — and an operator
    /// installing twice is asking for the second one.
    ///
    /// # Errors
    ///
    /// Returns a [`Failure`] where the platform has no manager, the definition
    /// could not be written, or the manager refused it. An install that got as
    /// far as writing a definition and no further removes it again, so a failure
    /// leaves nothing behind.
    async fn place(&self, hosted: &Hosted) -> Result<Placed, Failure>;

    /// What the manager holds under this name.
    ///
    /// # Errors
    ///
    /// Returns a [`Failure`] where the platform has no manager. A name nothing
    /// is installed under is [`Held::absent`] rather than an error: not being
    /// installed is an ordinary answer.
    async fn standing(&self, name: &str) -> Result<Held, Failure>;

    /// Take back everything installed under this name.
    ///
    /// Returns every path it removed, so a caller can say what went rather than
    /// asserting that something did.
    ///
    /// # Errors
    ///
    /// Returns a [`Failure`] where the platform has no manager, or where the
    /// manager would not release it. A name nothing is installed under removes
    /// nothing and succeeds.
    async fn withdraw(&self, name: &str) -> Result<Vec<PathBuf>, Failure>;
}

#[cfg(test)]
mod tests {
    use super::{Diagnose, Failure, Held, Manager, Program, Standing};
    use std::path::PathBuf;

    #[test]
    fn each_manager_is_named_the_way_its_own_documentation_names_it() {
        assert_eq!(Manager::Launchd.named(), "launchd");
        assert_eq!(Manager::Systemd.named(), "systemd");
        assert_eq!(Manager::Unsupported.named(), "none");
    }

    #[test]
    fn only_a_manager_lemonfiber_speaks_to_can_be_configured() {
        assert!(Manager::Launchd.configurable());
        assert!(Manager::Systemd.configurable());
        assert!(!Manager::Unsupported.configurable());
    }

    #[test]
    fn nothing_installed_is_an_answer_rather_than_an_absence_of_one() {
        let held = Held::absent();
        assert_eq!(held.standing, Standing::Absent);
        assert_eq!(held.definition, None);
        assert_eq!(held.program, None);
        assert_eq!(held.runs, None);
        assert_eq!(held.output, None);
        assert!(!held.orphaned());
    }

    #[test]
    fn only_a_definition_naming_a_program_that_has_gone_is_orphaned() {
        let against = |present| Held {
            program: Some(Program {
                at: PathBuf::from("/usr/local/bin/lemonfiber"),
                present,
            }),
            ..Held::absent()
        };
        assert!(against(false).orphaned());
        assert!(!against(true).orphaned());
    }

    #[test]
    fn a_platform_with_no_manager_is_told_what_to_do_instead() {
        let problem = Failure::Unhostable.problem();
        assert_eq!(problem.code.as_str(), "HOST-1");
        assert!(!problem.remedies.is_empty());
        assert!(Failure::Unhostable
            .to_string()
            .contains("no service manager"));
    }

    #[test]
    fn a_definition_that_could_not_be_written_says_nothing_was_left_behind() {
        let failure = Failure::Unwritable {
            at: PathBuf::from("/nowhere/lemonfiber-watch.plist"),
            reason: "permission denied".to_owned(),
        };
        let problem = failure.problem();
        assert_eq!(problem.code.as_str(), "HOST-2");
        assert!(problem.summary.contains("/nowhere/lemonfiber-watch.plist"));
        assert_eq!(problem.detail.as_deref(), Some("permission denied"));
        assert!(failure.to_string().contains("permission denied"));
    }

    #[test]
    fn a_manager_that_refused_is_quoted_and_says_the_definition_went_too() {
        let failure = Failure::Refused {
            manager: "launchd",
            reason: "Load failed: 5: Input/output error".to_owned(),
        };
        let problem = failure.problem();
        assert_eq!(problem.code.as_str(), "HOST-3");
        assert!(problem.summary.contains("launchd"));
        assert!(problem.meaning.contains("removed again"));
        assert_eq!(
            problem.detail.as_deref(),
            Some("Load failed: 5: Input/output error")
        );
        assert!(failure.to_string().contains("launchd"));
    }
}
