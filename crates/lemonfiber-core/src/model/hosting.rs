//! What this machine keeps running on lemonfiber's behalf, and what a change to it did.
//!
//! One of the report families the machine-readable contract is made of; they live in
//! separate files and are re-exported as one, so `crate::model::X` reads the same as it
//! always did.

use std::path::PathBuf;

use serde::Serialize;

use crate::ports::hosting::Manager;

/// What stands between one long-running command and the machine.
///
/// Written with hyphens because these are the words the operator reads and the
/// requirement names, and a reading that spelled them differently would be a
/// second vocabulary for one set of facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Hosting {
    /// Nothing is installed; it runs only while a terminal holds it.
    NotHosted,
    /// Installed, and the manager confirms it is running.
    Hosted,
    /// Installed, and the manager would not say whether it is running.
    InstalledUnverified,
    /// Installed, and the manager says it is not running.
    Stopped,
    /// Installed against a program that is no longer there.
    Orphaned,
    /// This platform has no service manager lemonfiber configures.
    Unsupported,
}

impl Hosting {
    /// Whether anything is installed for it at all.
    #[must_use]
    pub const fn installed(self) -> bool {
        matches!(
            self,
            Self::Hosted | Self::InstalledUnverified | Self::Stopped | Self::Orphaned
        )
    }

    /// Whether this machine is keeping the guarantee the command makes.
    ///
    /// Only one of the six answers yes, which is the point of there being six:
    /// installed, stopped, orphaned and unverified are four different reasons a
    /// guarantee an operator believes is in force is not.
    #[must_use]
    pub const fn keeping(self) -> bool {
        matches!(self, Self::Hosted)
    }
}

/// One long-running command, and what stands between it and this machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct HostedCommand {
    /// lemonfiber's own name for it.
    pub name: String,
    /// What it does for as long as it runs, in one sentence.
    pub guarantees: String,
    /// How it is typed in a terminal, which is what hosting installs.
    pub command: String,
    /// What stands between it and the machine.
    pub standing: Hosting,
    /// The service definition installed for it, where there is one.
    pub definition: Option<PathBuf>,
    /// The whole command line that definition runs.
    pub runs: Option<String>,
    /// Where a hosted run writes the words it would have said on a terminal.
    pub output: Option<PathBuf>,
    /// The program the definition names, where nothing is there any more.
    pub missing: Option<PathBuf>,
}

/// What one run of this command did to the machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Changed {
    /// The command it acted on.
    pub name: String,
    /// Whether it installed it, rather than took it back.
    pub installed: bool,
    /// Everything it wrote or removed, so nothing goes unnamed in either direction.
    pub touched: Vec<PathBuf>,
    /// Whether it started the command, which only installing does.
    pub started: bool,
    /// Whether this was a rehearsal, in which case nothing above happened.
    pub rehearsed: bool,
}

/// What this machine keeps running on lemonfiber's behalf.
///
/// Defaultable so a test can read one out of a `Result` without a branch it can
/// never take: a closure standing in for the impossible arm is a region no
/// passing run enters, and the coverage gate counts those.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct HostingReport {
    /// The service manager this platform has, or the absence of one.
    pub manager: Manager,
    /// Every long-running command there is, hosted or not.
    pub commands: Vec<HostedCommand>,
    /// What this run changed, where it was asked to change something.
    pub changed: Option<Changed>,
    /// What to do instead, where lemonfiber cannot configure this platform.
    pub instruction: Option<String>,
    /// What is true of this manager and worth knowing before it is relied on.
    pub caveat: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{HostedCommand, Hosting};

    #[test]
    fn the_states_are_spelled_the_way_the_operator_reads_them() {
        let named = |standing| {
            serde_json::to_value(HostedCommand {
                name: "watch".to_owned(),
                guarantees: "guards the data location".to_owned(),
                command: "lemonfiber watch".to_owned(),
                standing,
                definition: None,
                runs: None,
                output: None,
                missing: None,
            })
            .ok()
            .and_then(|value| {
                value
                    .get("standing")
                    .and_then(|at| at.as_str())
                    .map(str::to_owned)
            })
        };
        assert_eq!(named(Hosting::NotHosted).as_deref(), Some("not-hosted"));
        assert_eq!(named(Hosting::Hosted).as_deref(), Some("hosted"));
        assert_eq!(
            named(Hosting::InstalledUnverified).as_deref(),
            Some("installed-unverified")
        );
        assert_eq!(named(Hosting::Stopped).as_deref(), Some("stopped"));
        assert_eq!(named(Hosting::Orphaned).as_deref(), Some("orphaned"));
        assert_eq!(named(Hosting::Unsupported).as_deref(), Some("unsupported"));
    }

    #[test]
    fn four_of_the_six_are_installed_and_only_one_of_them_is_keeping_anything() {
        for standing in [
            Hosting::Hosted,
            Hosting::InstalledUnverified,
            Hosting::Stopped,
            Hosting::Orphaned,
        ] {
            assert!(standing.installed(), "{standing:?} has a definition");
        }
        for standing in [Hosting::NotHosted, Hosting::Unsupported] {
            assert!(!standing.installed(), "{standing:?} has none");
        }
        assert!(Hosting::Hosted.keeping());
        for standing in [
            Hosting::NotHosted,
            Hosting::InstalledUnverified,
            Hosting::Stopped,
            Hosting::Orphaned,
            Hosting::Unsupported,
        ] {
            assert!(!standing.keeping(), "{standing:?} keeps nothing");
        }
    }
}
