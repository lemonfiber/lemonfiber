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

use crate::migration::version::{among_versions, Standing};

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
///
/// The second half is conditional and has to be, because the requirement is. Saying
/// "a release brings its own stack" is true of every release and therefore tells an
/// operator nothing about the one in front of them; what they are deciding about is
/// whether *this* update brings a stack description their current copy cannot read,
/// and what that implies for the services. So the generation the offered release
/// declares is compared against the ones this copy reads, and the three answers are
/// different sentences rather than one hedged one.
///
/// `offered` is absent where the release declared nothing — published before the
/// declaration existed, or by a fork that publishes none. That is said plainly rather
/// than guessed at: claiming a release brings no newer schema when nobody asked it is
/// how an operator ends up surprised by the thing this sentence exists to prevent.
#[must_use]
pub fn carries(reads: &[u32], offered: Option<u32>) -> String {
    let generations: Vec<String> = reads.iter().map(u32::to_string).collect();
    let generations = generations.join(", ");
    let brings = match offered {
        Some(offered) if !reads.contains(&offered) => format!(
            " The release on offer declares manifest schema {offered}, which this copy does not \
             read — so it brings a newer stack description as well as a newer program, and the \
             service versions pinned inside it move with it. Starting the stack after the update \
             is what fetches those images."
        ),
        Some(offered) => format!(
            " The release on offer declares manifest schema {offered}, which this copy already \
             reads, so the update brings no newer stack description."
        ),
        None => " The release on offer declares no manifest schema, so whether it brings a newer \
                 stack description than this copy reads cannot be told from here."
            .to_owned(),
    };
    format!(
        "A release carries its own pinned stack as well as the program, and this copy reads \
         manifest schema {generations}. Updating writes none of it: the copy you install \
         materialises its own stack the next time you start one, and where that stack pins newer \
         service images, starting it is what fetches them.{brings}"
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
    match among_versions(named, running) {
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
mod tests;
