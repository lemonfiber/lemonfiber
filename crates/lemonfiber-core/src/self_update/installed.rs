//! Which tool, if any, owns the copy of lemonfiber that is running.
//!
//! A single binary reaches a machine by five or six different roads, and the roads
//! disagree about who owns the file afterwards. Homebrew keeps a record of what it
//! put where and reconciles it on every `brew upgrade`; a binary that replaced
//! itself underneath that record leaves the next upgrade producing something the
//! operator did not ask for and cannot explain. So the question this answers is not
//! *can lemonfiber write to its own path* — it usually can — but *whose file is
//! this*, and the answer decides whether updating is lemonfiber's to do at all.
//!
//! Decided from what the machine says rather than from a build flag, because the
//! same artefact reaches all of these roads: the binary in a Homebrew cellar and the
//! one a shell installer wrote are byte-identical. Everything read is passed in, for
//! the reason [`crate::platform::Environment::resolve`] takes its inputs — all seven
//! answers are reachable from one laptop, so what this says on a machine nobody here
//! has is proven rather than hoped for.

use std::path::Path;

use serde::Serialize;

/// How this copy of lemonfiber got onto the machine.
///
/// Nothing known is the default. Every other answer is a claim about somebody's
/// machine, and a value that arrived by nobody filling it in has established none of
/// them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Installed {
    /// Homebrew put it here, on macOS or on Linux.
    Homebrew,
    /// Scoop put it here.
    Scoop,
    /// winget put it here.
    Winget,
    /// `cargo install` built and placed it.
    Cargo,
    /// A distribution's own package manager placed it.
    Distribution,
    /// The shell installer this project publishes wrote it.
    Installer,
    /// Downloaded, unpacked or built by hand, and owned by nobody but the operator.
    Elsewhere,
    /// This machine would not say where the running binary is, so none of the above
    /// can be told apart. Not a failure, and not a licence to guess.
    #[default]
    Untellable,
}

/// What this machine says about the copy that is running.
///
/// Three facts, and each is a read somewhere else does: the path is the platform's
/// answer for the running executable, and the two receipts are files that either sit
/// where a tool writes one or do not. Held as data so every answer below is reachable
/// without the tool that produces it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Signs<'a> {
    /// Where the running binary is, with any symbolic link followed.
    ///
    /// Following the link is what makes Homebrew tellable at all: `brew` installs
    /// into a cellar and links the name into its `bin`, so the unresolved path says
    /// only that something is on the path and the resolved one says whose it is.
    pub at: Option<&'a Path>,
    /// Whether the shell installer's own receipt sits where it writes one.
    pub receipt: bool,
    /// Whether cargo's record of what it installed names this program.
    pub recorded_by_cargo: bool,
}

impl Installed {
    /// Read which tool owns this copy from what the machine says.
    ///
    /// Ordered by how specific the evidence is. A path inside a package manager's
    /// own tree is that manager's and nothing else's, so those come first; the two
    /// receipts settle the one case a path cannot, since the shell installer writes
    /// into cargo's `bin` directory by default and a binary there could have arrived
    /// either way. The receipt is read before cargo's record because the installer
    /// writes one on every run while cargo does not remove its record when something
    /// else overwrites the file, so of the two the receipt is the later truth.
    #[must_use]
    pub fn read(signs: &Signs) -> Self {
        let Some(at) = signs.at else {
            return Self::Untellable;
        };
        if named(at, "Cellar") || named(at, "homebrew") || named(at, "linuxbrew") {
            return Self::Homebrew;
        }
        if named(at, "scoop") {
            return Self::Scoop;
        }
        if named(at, "WinGet") || named(at, "WindowsApps") {
            return Self::Winget;
        }
        if signs.receipt {
            return Self::Installer;
        }
        if signs.recorded_by_cargo {
            return Self::Cargo;
        }
        if distributed(at) {
            return Self::Distribution;
        }
        Self::Elsewhere
    }

    /// The tool that owns this copy, in the words an operator calls it by.
    ///
    /// Nothing where no tool owns it, which is the case where updating is
    /// lemonfiber's own to do rather than somebody else's.
    #[must_use]
    pub const fn owner(self) -> Option<&'static str> {
        match self {
            Self::Homebrew => Some("Homebrew"),
            Self::Scoop => Some("Scoop"),
            Self::Winget => Some("winget"),
            Self::Cargo => Some("Cargo"),
            Self::Distribution => Some("this system's own package manager"),
            Self::Installer | Self::Elsewhere | Self::Untellable => None,
        }
    }

    /// Whether this tool finds the newest release for itself, given only a name.
    ///
    /// Four of them keep an index and are asked for a package by name; the rest fetch
    /// one particular artefact and have to be told which one. Naming a version to a
    /// tool that keeps an index asks it for one the index may not carry, and it
    /// answers with nothing rather than with the upgrade it could have done.
    #[must_use]
    pub const fn resolves_newest(self) -> bool {
        matches!(
            self,
            Self::Homebrew | Self::Scoop | Self::Winget | Self::Distribution
        )
    }

    /// Whether updating this copy belongs to somebody else.
    ///
    /// Deferring is not a failure and is not a lesser outcome. Fighting the tool
    /// that owns the file is the failure, and it is the one an operator finds out
    /// about at their next upgrade rather than now.
    #[must_use]
    pub const fn defers(self) -> bool {
        self.owner().is_some()
    }

    /// The name this is reported and asked about by.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Homebrew => "homebrew",
            Self::Scoop => "scoop",
            Self::Winget => "winget",
            Self::Cargo => "cargo",
            Self::Distribution => "distribution",
            Self::Installer => "installer",
            Self::Elsewhere => "elsewhere",
            Self::Untellable => "untellable",
        }
    }
}

/// Every way a copy can have arrived, in the order they are told apart.
pub const EVERY_WAY: &[Installed] = &[
    Installed::Homebrew,
    Installed::Scoop,
    Installed::Winget,
    Installed::Cargo,
    Installed::Distribution,
    Installed::Installer,
    Installed::Elsewhere,
    Installed::Untellable,
];

/// The directories a path names, splitting on either platform's separator.
///
/// Both separators from every machine, rather than the running platform's, and that
/// is the point rather than laxity: a Windows path read here on a laptop would
/// otherwise be one long name with no directories in it, so the two answers told
/// apart by a Windows convention would be unreachable from any machine a test runs
/// on. Neither separator is legal inside a directory name on the platform that uses
/// the other, so nothing is confused by reading for both.
fn parts(path: &Path) -> impl Iterator<Item = &str> {
    path.to_str()
        .unwrap_or_default()
        .split(['/', '\\'])
        .filter(|part| !part.is_empty())
}

/// Whether a path holds a directory of this name, whatever its case.
///
/// Case-insensitively because two of these are Windows conventions and Windows
/// paths are written however the writer felt; the rest are lower-case on every
/// machine that has them, so nothing is loosened by asking the same way.
fn named(path: &Path, directory: &str) -> bool {
    parts(path).any(|part| part.eq_ignore_ascii_case(directory))
}

/// Whether a path is where a distribution's package manager puts a program.
///
/// The system's own `bin` and nothing under it. `/usr/local/bin` is deliberately
/// not here: it is the directory the operating system reserves for what the
/// operator installed themselves, so a binary there is theirs rather than a
/// package manager's, and telling them to reach for `apt` would send them after a
/// package that does not exist.
fn distributed(path: &Path) -> bool {
    let held: Vec<&str> = parts(path).collect();
    matches!(held.as_slice(), ["usr", "bin", _] | ["bin", _])
}

#[cfg(test)]
mod tests {
    use super::{Installed, Signs, EVERY_WAY};
    use std::path::{Path, PathBuf};

    /// The signs a machine gives for a binary at this path and nothing else.
    fn at(path: &str) -> Signs<'_> {
        Signs {
            at: Some(Path::new(path)),
            ..Signs::default()
        }
    }

    #[test]
    fn a_machine_that_will_not_say_where_the_binary_is_is_not_guessed_at() {
        assert_eq!(Installed::read(&Signs::default()), Installed::Untellable);
        assert_eq!(Installed::Untellable.owner(), None);
        assert!(!Installed::Untellable.defers());
    }

    #[test]
    fn a_binary_in_a_cellar_belongs_to_homebrew_on_either_platform() {
        for path in [
            "/opt/homebrew/Cellar/lemonfiber/0.13.0/bin/lemonfiber",
            "/usr/local/Cellar/lemonfiber/0.13.0/bin/lemonfiber",
            "/home/linuxbrew/.linuxbrew/bin/lemonfiber",
        ] {
            assert_eq!(Installed::read(&at(path)), Installed::Homebrew, "{path}");
        }
    }

    #[test]
    fn the_windows_managers_are_told_apart_by_the_tree_they_install_into() {
        assert_eq!(
            Installed::read(&at(
                r"C:\Users\sam\scoop\apps\lemonfiber\current\lemonfiber.exe"
            )),
            Installed::Scoop
        );
        for path in [
            r"C:\Users\sam\AppData\Local\Microsoft\WinGet\Packages\lemonfiber\lemonfiber.exe",
            r"C:\Program Files\WindowsApps\lemonfiber\lemonfiber.exe",
        ] {
            assert_eq!(Installed::read(&at(path)), Installed::Winget, "{path}");
        }
    }

    /// The one case a path cannot answer: the shell installer's default location is
    /// cargo's own `bin`, so a binary there arrived either way and only a receipt
    /// says which.
    #[test]
    fn the_two_that_share_a_directory_are_told_apart_by_their_receipts() {
        let path = PathBuf::from("/home/sam/.cargo/bin/lemonfiber");
        let bare = Signs {
            at: Some(&path),
            ..Signs::default()
        };
        assert_eq!(Installed::read(&bare), Installed::Elsewhere);
        assert_eq!(
            Installed::read(&Signs {
                recorded_by_cargo: true,
                ..bare
            }),
            Installed::Cargo
        );
        assert_eq!(
            Installed::read(&Signs {
                receipt: true,
                ..bare
            }),
            Installed::Installer
        );
    }

    /// Both receipts present is a machine where one tool wrote over the other's
    /// work, and the installer is the one that leaves a receipt every time.
    #[test]
    fn a_receipt_from_each_is_read_as_the_one_that_writes_one_every_time() {
        let path = PathBuf::from("/home/sam/.cargo/bin/lemonfiber");
        assert_eq!(
            Installed::read(&Signs {
                at: Some(&path),
                receipt: true,
                recorded_by_cargo: true,
            }),
            Installed::Installer
        );
    }

    #[test]
    fn a_package_managers_tree_is_read_before_any_receipt() {
        let path = PathBuf::from("/opt/homebrew/Cellar/lemonfiber/0.13.0/bin/lemonfiber");
        assert_eq!(
            Installed::read(&Signs {
                at: Some(&path),
                receipt: true,
                recorded_by_cargo: true,
            }),
            Installed::Homebrew
        );
    }

    #[test]
    fn the_systems_own_bin_is_a_distribution_and_the_one_beside_it_is_not() {
        assert_eq!(
            Installed::read(&at("/usr/bin/lemonfiber")),
            Installed::Distribution
        );
        assert_eq!(
            Installed::read(&at("/bin/lemonfiber")),
            Installed::Distribution
        );
        assert_eq!(
            Installed::read(&at("/usr/local/bin/lemonfiber")),
            Installed::Elsewhere
        );
    }

    #[test]
    fn a_binary_nobody_claims_is_the_operators_own() {
        for path in [
            "/home/sam/Downloads/lemonfiber",
            "/Users/sam/code/lemonfiber/target/release/lemonfiber",
            "lemonfiber",
        ] {
            assert_eq!(Installed::read(&at(path)), Installed::Elsewhere, "{path}");
        }
    }

    #[test]
    fn the_ones_a_tool_owns_are_the_ones_that_defer_and_they_name_the_tool() {
        let deferring: Vec<Installed> = EVERY_WAY
            .iter()
            .copied()
            .filter(|way| way.defers())
            .collect();
        assert_eq!(
            deferring,
            vec![
                Installed::Homebrew,
                Installed::Scoop,
                Installed::Winget,
                Installed::Cargo,
                Installed::Distribution,
            ]
        );
        for way in EVERY_WAY {
            assert_eq!(way.defers(), way.owner().is_some(), "{way:?}");
        }
    }

    /// The ones that keep an index are exactly the ones a version is never named to.
    /// A tool told a version its index does not carry answers with nothing, which
    /// leaves an operator holding a report and no way forward.
    #[test]
    fn the_ones_that_keep_an_index_are_the_ones_that_find_the_newest_themselves() {
        let finding: Vec<Installed> = EVERY_WAY
            .iter()
            .copied()
            .filter(|way| way.resolves_newest())
            .collect();
        assert_eq!(
            finding,
            vec![
                Installed::Homebrew,
                Installed::Scoop,
                Installed::Winget,
                Installed::Distribution,
            ]
        );
    }

    #[test]
    fn every_way_is_named_once_and_by_something_a_reader_can_type() {
        let named: Vec<&str> = EVERY_WAY.iter().map(|way| way.as_str()).collect();
        assert_eq!(
            named,
            vec![
                "homebrew",
                "scoop",
                "winget",
                "cargo",
                "distribution",
                "installer",
                "elsewhere",
                "untellable"
            ]
        );
    }

    #[test]
    fn a_tree_is_recognised_however_its_case_was_written() {
        assert_eq!(
            Installed::read(&at(r"C:\Users\sam\SCOOP\apps\lemonfiber\lemonfiber.exe")),
            Installed::Scoop
        );
    }
}
