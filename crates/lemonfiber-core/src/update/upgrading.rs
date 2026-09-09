//! What to type to move this copy of lemonfiber to another version.
//!
//! One line per way of having installed it, and the line is the whole of what
//! deferring comes to: an operator told "this was installed by Homebrew" and left
//! there has been given a fact rather than a way forward, and will reach for the
//! thing that overwrites the file.
//!
//! Two of these are exact only when a version is known, and that is a property of
//! how this project releases rather than an oversight. Every release is published as
//! a pre-release, so the endpoint that serves "the latest" has nothing to serve and
//! the address that redirects to the latest download does not resolve — a command
//! naming no version would be a command that fails. Where the version cannot be
//! read, nothing is printed rather than something that will not work.

use crate::migration::version::{against, Standing};

use super::installed::Installed;

/// Where this project's own releases are published.
///
/// Named here because it is what an operator is told to fetch from rather than
/// something lemonfiber asks for; where the release list is *asked* for is declared
/// in [`crate::outbound`], which is the list an operator can read and switch off.
const RELEASES: &str = "https://github.com/lemonfiber/lemonfiber";

/// What to type to bring this copy to `version`, or to whatever is newest where no
/// version is named.
///
/// Nothing where the tool that owns this copy has no command for what was asked,
/// which is a real answer and not a gap: Homebrew installs whatever its tap carries
/// and has no general form for an arbitrary older version, and no distribution
/// carries lemonfiber at all. [`why_not`] says which of those it was.
#[must_use]
pub fn command(installed: Installed, version: Option<&str>) -> Option<String> {
    let product = crate::PRODUCT;
    match (installed, version) {
        (Installed::Homebrew, None) => Some(format!("brew upgrade {product}")),
        (Installed::Scoop, None) => Some(format!("scoop update {product}")),
        (Installed::Scoop, Some(version)) => Some(format!("scoop install {product}@{version}")),
        (Installed::Winget, None) => Some(format!("winget upgrade {product}")),
        (Installed::Winget, Some(version)) => {
            Some(format!("winget install --version {version} {product}"))
        }
        (Installed::Cargo, Some(version)) => Some(format!(
            "cargo install --git {RELEASES} --tag v{version} --force {product}"
        )),
        (Installed::Installer | Installed::Elsewhere | Installed::Untellable, Some(version)) => {
            Some(format!(
                "curl -LsSf {RELEASES}/releases/download/v{version}/{product}-installer.sh | sh"
            ))
        }
        (Installed::Homebrew | Installed::Distribution, _)
        | (
            Installed::Cargo | Installed::Installer | Installed::Elsewhere | Installed::Untellable,
            None,
        ) => None,
    }
}

/// Why there is nothing exact to type, where there is not.
///
/// A sentence rather than silence. The two reasons are different in kind — one is
/// about the tool that owns the file and one is about not knowing which version to
/// name — and an operator can act on either once they are told which they have hit.
#[must_use]
pub fn why_not(installed: Installed, version: Option<&str>) -> Option<&'static str> {
    if command(installed, version).is_some() {
        return None;
    }
    match installed {
        Installed::Homebrew => Some(
            "Homebrew installs whatever version its tap carries, and has no command for an \
             older one. Remove it with `brew uninstall lemonfiber` and install the version \
             you want another way.",
        ),
        Installed::Distribution => Some(
            "No distribution carries lemonfiber, so the package manager that placed this \
             copy is one somebody here set up. Ask it for the update the way it was asked \
             for the install.",
        ),
        _ => Some(
            "Which version to move to is not known, and every release of lemonfiber is \
             published as a pre-release — so a command naming no version would ask for one \
             that is not served. Run this again when the check can reach the release list.",
        ),
    }
}

/// What updating does to the stack, said every time rather than when asked.
///
/// Both halves are worth saying and neither is obvious. Containers do not run
/// inside this program — it starts them and then has nothing to do with them — so
/// replacing the binary is invisible to a stack that is up, and an operator who
/// assumed otherwise would put off updating until a quiet week they never get. The
/// second half is the cost of the first: whatever replaces the file cannot reach the
/// copy already in memory, so the version running is the old one until it is started
/// again.
pub const AFTERWARDS: &str = "Nothing in the stack is stopped, restarted or changed by this: \
                              containers run on their own and this program only starts them. \
                              The copy already running stays the old one until you run it \
                              again.";

/// What a release brings with it besides the program, said before anything is typed.
///
/// A release carries a pinned stack description as well as a binary, and an operator
/// who reads only "a newer version exists" would not know that. Nothing is written by
/// taking one: the copy that is installed materialises its own stack the next time it
/// is asked to start one, and where that stack pins newer service images, starting it
/// is what fetches them.
#[must_use]
pub fn carries(reads: &[u32]) -> String {
    let generations: Vec<String> = reads.iter().map(u32::to_string).collect();
    format!(
        "A release carries its own pinned stack as well as the program, and this copy reads \
         manifest schema {}. Updating writes none of it: the copy you install materialises \
         its own stack the next time you start one, and where that stack pins newer service \
         images, starting it is what fetches them.",
        generations.join(", ")
    )
}

/// Whether the version named can read the configuration on this machine.
///
/// Answered rather than measured, and that is a property of the product rather than a
/// shortcut: settings are names and values in one file, nothing here migrates one way,
/// and a name a copy does not recognise is left alone rather than rewritten. Where the
/// two cannot be ordered, that is said instead of a claim about which came first.
#[must_use]
pub fn configuration(named: &str, running: &str) -> String {
    match against(named, running) {
        Standing::Earlier => format!(
            "{named} is behind the copy running, and reads this machine's configuration as it \
             stands. Settings are names and values in one file, nothing lemonfiber keeps \
             migrates one way, and a name an older copy does not recognise is left alone \
             rather than rewritten — which is what makes going back safe."
        ),
        Standing::Same => format!("{named} is the version already running."),
        Standing::Later => format!(
            "{named} is ahead of the copy running, and reads this machine's configuration as \
             it stands. What it writes afterwards may hold names this copy does not know."
        ),
        Standing::Untellable => format!(
            "Whether {named} reads this machine's configuration cannot be said: it and the \
             copy running cannot be ordered against each other."
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{carries, command, configuration, why_not, AFTERWARDS};
    use crate::update::installed::{Installed, EVERY_WAY};

    #[test]
    fn each_manager_is_asked_in_its_own_words_for_whatever_is_newest() {
        assert_eq!(
            command(Installed::Homebrew, None).as_deref(),
            Some("brew upgrade lemonfiber")
        );
        assert_eq!(
            command(Installed::Scoop, None).as_deref(),
            Some("scoop update lemonfiber")
        );
        assert_eq!(
            command(Installed::Winget, None).as_deref(),
            Some("winget upgrade lemonfiber")
        );
    }

    #[test]
    fn a_named_version_is_asked_for_by_the_managers_that_take_one() {
        assert_eq!(
            command(Installed::Scoop, Some("0.12.0")).as_deref(),
            Some("scoop install lemonfiber@0.12.0")
        );
        assert_eq!(
            command(Installed::Winget, Some("0.12.0")).as_deref(),
            Some("winget install --version 0.12.0 lemonfiber")
        );
    }

    /// This project is not published to a registry, so the only thing cargo can be
    /// pointed at is the repository and a tag — which is why it needs a version
    /// where the two managers above do not.
    #[test]
    fn cargo_is_pointed_at_the_repository_and_a_tag_because_there_is_no_registry_to_name() {
        let said = command(Installed::Cargo, Some("0.13.0"));
        let asked = said.unwrap_or_default();
        assert!(
            asked.starts_with("cargo install --git https://github.com/"),
            "{asked}"
        );
        assert!(asked.contains("--tag v0.13.0"), "{asked}");
        assert!(asked.ends_with("lemonfiber"), "{asked}");
        assert_eq!(command(Installed::Cargo, None), None);
    }

    #[test]
    fn the_three_that_nobody_owns_are_sent_back_to_the_installer_for_that_version() {
        for way in [
            Installed::Installer,
            Installed::Elsewhere,
            Installed::Untellable,
        ] {
            let said = command(way, Some("0.13.0"));
            let asked = said.unwrap_or_default();
            assert!(
                asked.contains("releases/download/v0.13.0/"),
                "{way:?}: {asked}"
            );
            assert!(asked.ends_with("| sh"), "{way:?}: {asked}");
            assert_eq!(command(way, None), None, "{way:?}");
        }
    }

    #[test]
    fn homebrew_has_no_form_for_an_older_version_and_says_which_reason_that_is() {
        assert_eq!(command(Installed::Homebrew, Some("0.12.0")), None);
        let said = why_not(Installed::Homebrew, Some("0.12.0")).unwrap_or_default();
        assert!(said.contains("brew uninstall"), "{said}");
    }

    #[test]
    fn a_distribution_is_told_to_ask_whatever_set_it_up() {
        for version in [None, Some("0.12.0")] {
            assert_eq!(command(Installed::Distribution, version), None);
            let said = why_not(Installed::Distribution, version).unwrap_or_default();
            assert!(said.contains("No distribution carries"), "{said}");
        }
    }

    #[test]
    fn not_knowing_which_version_is_said_as_that_rather_than_as_the_tools_fault() {
        let said = why_not(Installed::Installer, None).unwrap_or_default();
        assert!(said.contains("pre-release"), "{said}");
    }

    /// Exactly one of the two is present for every combination. A way with neither
    /// leaves an operator nothing, and one with both would be a command printed
    /// beside a sentence saying there is no command.
    #[test]
    fn every_way_answers_with_a_command_or_with_why_there_is_none() {
        for way in EVERY_WAY {
            for version in [None, Some("0.12.0")] {
                let typed = command(*way, version);
                let refused = why_not(*way, version);
                assert_eq!(
                    typed.is_some(),
                    refused.is_none(),
                    "{way:?} with {version:?} answered {typed:?} and {refused:?}"
                );
            }
        }
    }

    #[test]
    fn what_a_release_brings_besides_the_program_is_said_with_what_this_copy_reads() {
        let said = carries(&[1, 2]);
        assert!(said.contains("manifest schema 1, 2"), "{said}");
        assert!(said.contains("newer service images"), "{said}");
        assert!(said.contains("Updating writes none of it"), "{said}");
    }

    /// The question a downgrade asks, and the one the specification says is
    /// answerable: whether the version being moved to reads what is on this machine.
    #[test]
    fn a_version_behind_this_one_is_said_to_read_what_is_already_here() {
        let said = configuration("0.12.0", "0.13.0");
        assert!(said.starts_with("0.12.0 is behind"), "{said}");
        assert!(said.contains("migrates one way"), "{said}");
    }

    #[test]
    fn a_version_ahead_and_the_version_running_are_each_said_as_what_they_are() {
        assert!(configuration("0.14.0", "0.13.0").contains("ahead of the copy running"));
        assert_eq!(
            configuration("0.13.0", "0.13.0"),
            "0.13.0 is the version already running."
        );
    }

    /// Two versions nothing can order are not guessed at. Telling somebody an
    /// unorderable version reads their configuration would be a claim made by an
    /// ordering that never ran.
    #[test]
    fn two_versions_that_cannot_be_ordered_claim_nothing_about_the_configuration() {
        let said = configuration("nightly", "0.13.0");
        assert!(said.contains("cannot be ordered"), "{said}");
    }

    #[test]
    fn what_updating_leaves_alone_is_said_along_with_what_it_costs() {
        assert!(AFTERWARDS.contains("Nothing in the stack is stopped"));
        assert!(AFTERWARDS.contains("until you run it again"));
    }
}
