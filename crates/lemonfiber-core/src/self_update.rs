//! Keeping lemonfiber current without fighting whatever put it here.
//!
//! One binary reaches a machine by half a dozen roads, and the roads disagree about
//! who owns the file afterwards. A copy that replaced itself underneath a package
//! manager leaves that manager holding a record of something that is no longer
//! there, and the operator finds out at their next upgrade, from a failure that
//! names neither this program nor what it did. So the first question is not whether
//! a newer version exists but *whose file this is*, and the answer decides whether
//! updating is lemonfiber's to do at all. Deferring is not a lesser outcome.
//! Fighting the tool that owns the file is the failure.
//!
//! **Availability is advisory and nothing here is a precondition.** The check is a
//! read an operator asks for; it reaches the network at most once a day, gets
//! quieter each time nothing answers, and stops entirely after enough of those. A
//! machine with no route out is answered from what was last read, and where nothing
//! was ever read the answer is that it could not be told — never an error, and never
//! something another command waits on. The stack goes on running on an old
//! lemonfiber, which is the property that makes updating safe to put off.

mod installed;
mod noticing;
mod offered;
mod upgrading;

use std::path::{Path, PathBuf};

use serde::Serialize;

pub use installed::{Installed, Signs, EVERY_WAY};
pub use noticing::{Noticed, Silence};
pub use offered::{asking, changed, newest, standing as availability, Availability};
pub use upgrading::{carries, command, configuration, why_not, AFTERWARDS};

/// The name a probe file is given, so a run interrupted between creating it and
/// taking it away leaves something recognisable rather than something alarming.
const PROBE: &str = ".lemonfiber-can-write";

/// Where the record an installer leaves sits, given this machine's home directory.
///
/// The convention belongs to the installer this project publishes, which writes it
/// under the configuration directory whatever platform it is run on. A machine that
/// has moved that directory elsewhere reads as one with no record, and falls back to
/// what the path alone says — which never claims a tool owns the file, so the miss
/// is in the direction that cannot do harm.
#[must_use]
pub fn receipt_under(home: &Path) -> PathBuf {
    home.join(".config")
        .join(crate::PRODUCT)
        .join(format!("{}-receipt.json", crate::PRODUCT))
}

/// Where cargo keeps its record of what it installed, given the binary's own path.
///
/// Derived from the binary rather than from a setting, and that is what makes it
/// trustworthy: cargo puts what it builds in `bin` under its own home, so a record
/// one directory above the binary is the record for *that* binary rather than for a
/// second cargo somewhere else on the machine.
#[must_use]
pub fn cargo_record_above(binary: &Path) -> Option<PathBuf> {
    Some(binary.parent()?.parent()?.join(".crates2.json"))
}

/// Where a probe would be written to find out whether this binary could be replaced.
///
/// Beside the binary rather than on it: replacing a file means writing a new one in
/// the same directory and moving it into place, so the directory is what has to be
/// writable and the binary itself is never touched by the asking.
#[must_use]
pub fn probe_beside(binary: &Path) -> Option<PathBuf> {
    Some(binary.parent()?.join(PROBE))
}

/// Where a copy of lemonfiber stands, in the words the specification uses.
///
/// Nothing known is the default, and it is the right one: a report built before
/// anything has been read has not established that this copy is current, and a
/// default that said so would be a claim made by an empty value.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Standing {
    /// The version running is the newest known.
    Current,
    /// A newer version exists, and replacing this copy is lemonfiber's own to do.
    UpdateAvailable,
    /// A newer version exists and a package manager owns this copy, so the update
    /// is that tool's to make.
    ManagedExternally,
    /// Availability could not be determined, which is not a fault and stops nothing.
    #[default]
    CheckFailed,
}

impl Standing {
    /// The name this is reported by.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::UpdateAvailable => "update-available",
            Self::ManagedExternally => "managed-externally",
            Self::CheckFailed => "check-failed",
        }
    }
}

/// Where this copy stands, from what the check came to and what owns the file.
///
/// Ordered so the useful answer wins. A copy a package manager owns and that is
/// already the newest is *current* — the tool that owns it is beside the point when
/// there is nothing to do — and only a copy with something to move to is reported as
/// somebody else's to move.
#[must_use]
pub fn stands(availability: Option<&Availability>, installed: Installed) -> Standing {
    match availability {
        None | Some(Availability::Untellable) => Standing::CheckFailed,
        Some(Availability::Current) => Standing::Current,
        Some(Availability::Newer(_)) if installed.defers() => Standing::ManagedExternally,
        Some(Availability::Newer(_)) => Standing::UpdateAvailable,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        cargo_record_above, probe_beside, receipt_under, stands, Availability, Installed, Standing,
    };
    use std::path::{Path, PathBuf};

    #[test]
    fn the_record_an_installer_leaves_is_looked_for_where_it_writes_one() {
        assert_eq!(
            receipt_under(Path::new("/home/sam")),
            PathBuf::from("/home/sam/.config/lemonfiber/lemonfiber-receipt.json")
        );
    }

    #[test]
    fn cargos_record_is_looked_for_above_the_directory_it_puts_binaries_in() {
        assert_eq!(
            cargo_record_above(Path::new("/home/sam/.cargo/bin/lemonfiber")),
            Some(PathBuf::from("/home/sam/.cargo/.crates2.json"))
        );
    }

    #[test]
    fn a_binary_with_nothing_above_it_has_no_record_to_look_for_and_nowhere_to_probe() {
        assert_eq!(cargo_record_above(Path::new("lemonfiber")), None);
        assert_eq!(probe_beside(Path::new("")), None);
    }

    #[test]
    fn a_probe_goes_beside_the_binary_and_never_on_it() {
        assert_eq!(
            probe_beside(Path::new("/usr/local/bin/lemonfiber")),
            Some(PathBuf::from("/usr/local/bin/.lemonfiber-can-write"))
        );
    }

    /// A version having been released since the one running.
    fn newer() -> Availability {
        Availability::Newer("0.14.0".to_owned())
    }

    #[test]
    fn a_check_that_could_not_tell_stops_nothing_and_claims_nothing() {
        assert_eq!(stands(None, Installed::Homebrew), Standing::CheckFailed);
        assert_eq!(
            stands(Some(&Availability::Untellable), Installed::Elsewhere),
            Standing::CheckFailed
        );
    }

    #[test]
    fn nothing_newer_is_current_whoever_owns_the_file() {
        for way in super::EVERY_WAY {
            assert_eq!(
                stands(Some(&Availability::Current), *way),
                Standing::Current,
                "{way:?}"
            );
        }
    }

    #[test]
    fn something_newer_under_a_package_manager_is_that_managers_to_do() {
        assert_eq!(
            stands(Some(&newer()), Installed::Homebrew),
            Standing::ManagedExternally
        );
    }

    #[test]
    fn something_newer_under_nobody_is_this_products_own_to_do() {
        for way in [
            Installed::Installer,
            Installed::Elsewhere,
            Installed::Untellable,
        ] {
            assert_eq!(
                stands(Some(&newer()), way),
                Standing::UpdateAvailable,
                "{way:?}"
            );
        }
    }

    /// A report nobody filled in has established nothing. Every other answer is a
    /// claim about somebody's machine, and a value that arrived by nobody filling it
    /// in has established none of them.
    #[test]
    fn a_report_nobody_filled_in_claims_nothing_about_this_machine() {
        let bare = crate::model::UpdateReport::default();

        assert_eq!(bare.standing, Standing::CheckFailed);
        assert_eq!(bare.installed, Installed::Untellable);
        assert_eq!(bare.command, None);
        assert_eq!(bare.at, None);
    }

    #[test]
    fn each_standing_is_named_by_the_word_the_specification_uses() {
        let named: Vec<&str> = [
            Standing::Current,
            Standing::UpdateAvailable,
            Standing::ManagedExternally,
            Standing::CheckFailed,
        ]
        .iter()
        .map(|standing| standing.as_str())
        .collect();
        assert_eq!(
            named,
            vec![
                "current",
                "update-available",
                "managed-externally",
                "check-failed"
            ]
        );
    }
}
