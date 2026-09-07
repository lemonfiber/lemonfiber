//! What is on this machine because of lemonfiber and is not lemonfiber's to remove.
//!
//! The container engine, a media server the operator installed natively, a tunnel
//! client, the binary itself. None of them was put there by an uninstall and none of
//! them comes off with one — but an operator who ran an uninstall and was told
//! nothing about them believes the machine is clean, and it is not.
//!
//! So each is named, with the reason it is not ours and the command that removes it
//! *on this platform*. A removal instruction for the wrong operating system is worse
//! than none: it reads as authoritative and does nothing.

use serde::Serialize;

use crate::platform::Environment;

/// Something an uninstall leaves behind, and how to remove it by hand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Outside {
    /// What it is.
    pub what: String,
    /// Why it is not lemonfiber's to take away.
    pub why: String,
    /// How to remove it on this platform, as the operator would type or do it.
    pub by_hand: String,
    /// Whether this machine was found to have it.
    ///
    /// A survey that could not look says nothing was found rather than that nothing
    /// is there, which is why the entry is listed either way and this field carries
    /// the difference.
    pub found: bool,
}

/// One thing beside the stack, with the words for it and where to look.
pub struct Beside {
    /// What it is.
    pub what: &'static str,
    /// Why it is not lemonfiber's.
    pub why: &'static str,
    /// How it comes off, per platform.
    pub by_hand: fn(Environment) -> &'static str,
    /// A well-known location this platform keeps it at, where there is one to look
    /// at. Nothing where the platform has no single place worth naming.
    pub at: fn(Environment) -> Option<&'static str>,
}

/// Everything an uninstall leaves behind that an operator would want to know about.
pub const EVERY: &[Beside] = &[
    Beside {
        what: "Docker itself",
        why: "lemonfiber runs on the container engine and did not install it. Removing it \
              would take every other project's containers with it.",
        by_hand: docker_by_hand,
        at: docker_at,
    },
    Beside {
        what: "a media server you installed natively",
        why: "Running the media server outside a container is something you chose and set \
              up yourself, so nothing here knows what it was told or what it wrote.",
        by_hand: jellyfin_by_hand,
        at: jellyfin_at,
    },
    Beside {
        what: "a tunnel client such as Tailscale",
        why: "Reaching this machine from outside the house is arranged with software you \
              installed, and other things on this machine may be reached through it.",
        by_hand: tailscale_by_hand,
        at: tailscale_at,
    },
    Beside {
        what: "the lemonfiber binary itself",
        why: "This program is running, and a program does not remove itself out from under \
              the command you are reading this in.",
        by_hand: binary_by_hand,
        at: binary_at,
    },
];

/// How the container engine comes off, per platform.
const fn docker_by_hand(environment: Environment) -> &'static str {
    match environment {
        Environment::MacOs => {
            "Quit Docker Desktop, then drag Docker from Applications to the Bin — or \
             `brew uninstall --cask docker` if you installed it that way"
        }
        Environment::Windows => "Uninstall Docker Desktop from Settings › Apps › Installed apps",
        Environment::LinuxDesktop => {
            "Uninstall Docker Desktop with your package manager — `sudo apt remove \
             docker-desktop` on Debian and Ubuntu"
        }
        Environment::LinuxNative | Environment::Unsupported => {
            "Remove the engine with your package manager — `sudo apt remove docker-ce \
             docker-ce-cli containerd.io` on Debian and Ubuntu"
        }
    }
}

/// Where the container engine sits, per platform.
const fn docker_at(environment: Environment) -> Option<&'static str> {
    match environment {
        Environment::MacOs => Some("/Applications/Docker.app"),
        Environment::LinuxNative | Environment::LinuxDesktop => Some("/usr/bin/dockerd"),
        Environment::Windows | Environment::Unsupported => None,
    }
}

/// How a natively-installed media server comes off, per platform.
const fn jellyfin_by_hand(environment: Environment) -> &'static str {
    match environment {
        Environment::MacOs => {
            "Quit Jellyfin, drag it from Applications to the Bin, and remove \
             ~/.local/share/jellyfin — which holds its own library database"
        }
        Environment::Windows => {
            "Uninstall Jellyfin from Settings › Apps › Installed apps, then remove the \
             jellyfin folder under your account's local application data, which holds \
             its own library database"
        }
        Environment::LinuxNative | Environment::LinuxDesktop | Environment::Unsupported => {
            "`sudo apt remove jellyfin` and then remove /var/lib/jellyfin, which holds \
             its own library database"
        }
    }
}

/// Where a natively-installed media server sits, per platform.
const fn jellyfin_at(environment: Environment) -> Option<&'static str> {
    match environment {
        Environment::MacOs => Some("/Applications/Jellyfin.app"),
        Environment::LinuxNative | Environment::LinuxDesktop => Some("/var/lib/jellyfin"),
        Environment::Windows | Environment::Unsupported => None,
    }
}

/// How a tunnel client comes off, per platform.
const fn tailscale_by_hand(environment: Environment) -> &'static str {
    match environment {
        Environment::MacOs => "Quit Tailscale and drag it from Applications to the Bin",
        Environment::Windows => "Uninstall Tailscale from Settings › Apps › Installed apps",
        Environment::LinuxNative | Environment::LinuxDesktop | Environment::Unsupported => {
            "`sudo tailscale down` and then `sudo apt remove tailscale`"
        }
    }
}

/// Where a tunnel client sits, per platform.
const fn tailscale_at(environment: Environment) -> Option<&'static str> {
    match environment {
        Environment::MacOs => Some("/Applications/Tailscale.app"),
        Environment::LinuxNative | Environment::LinuxDesktop => Some("/usr/sbin/tailscaled"),
        Environment::Windows | Environment::Unsupported => None,
    }
}

/// How the binary itself comes off, per platform.
const fn binary_by_hand(environment: Environment) -> &'static str {
    match environment {
        Environment::Windows => "Delete lemonfiber.exe from wherever you installed it",
        Environment::MacOs
        | Environment::LinuxNative
        | Environment::LinuxDesktop
        | Environment::Unsupported => {
            "`brew uninstall lemonfiber` if you installed it that way, or delete the \
             binary — `rm $(command -v lemonfiber)`"
        }
    }
}

/// The binary is wherever it was installed, which this cannot know.
const fn binary_at(_environment: Environment) -> Option<&'static str> {
    None
}

/// The location this platform keeps a thing at, for a survey to look at.
#[must_use]
pub fn looked_for(beside: &Beside, environment: Environment) -> Option<&'static str> {
    (beside.at)(environment)
}

/// One entry, said for this platform, with whether it was found.
#[must_use]
pub fn against(beside: &Beside, environment: Environment, found: bool) -> Outside {
    Outside {
        what: beside.what.to_owned(),
        why: beside.why.to_owned(),
        by_hand: (beside.by_hand)(environment).to_owned(),
        found,
    }
}

#[cfg(test)]
mod tests {
    use super::{against, looked_for, EVERY};
    use crate::platform::Environment;

    /// Every environment, so nothing here is written for one platform and untried on
    /// the rest.
    const PLATFORMS: [Environment; 5] = [
        Environment::MacOs,
        Environment::LinuxNative,
        Environment::LinuxDesktop,
        Environment::Windows,
        Environment::Unsupported,
    ];

    #[test]
    fn everything_left_behind_is_named_with_a_reason_and_a_way_to_remove_it() {
        assert!(EVERY.len() >= 4, "{} entries", EVERY.len());
        for environment in PLATFORMS {
            for beside in EVERY {
                let said = against(beside, environment, false);
                assert!(
                    said.by_hand.split_whitespace().count() >= 4,
                    "{} on {environment:?} says nothing an operator could do: {}",
                    said.what,
                    said.by_hand
                );
                assert!(
                    said.why.split_whitespace().count() >= 8,
                    "{} on {environment:?} does not say why it is not ours",
                    said.what
                );
            }
        }
    }

    /// The instructions are genuinely per-platform: the same entry read on macOS and
    /// on Windows must not hand over the same sentence, or "platform-specific" is a
    /// claim rather than a property.
    #[test]
    fn the_instructions_differ_between_the_platforms_they_are_written_for() {
        let same: Vec<&str> = EVERY
            .iter()
            .filter(|beside| {
                (beside.by_hand)(Environment::MacOs) == (beside.by_hand)(Environment::Windows)
            })
            .map(|beside| beside.what)
            .collect();

        assert!(
            same.is_empty(),
            "these hand a macOS operator a Windows instruction, or the reverse: {same:?}"
        );
    }

    /// Nothing here asks the operator to become somebody else in order to remove
    /// *lemonfiber*. Where a package manager needs administrative rights that is the
    /// package manager's business and is said as such, and it is never something this
    /// product runs.
    #[test]
    fn the_binary_and_the_engine_are_told_apart_by_where_they_are_looked_for() {
        for environment in PLATFORMS {
            let binary = EVERY
                .iter()
                .find(|beside| beside.what.contains("binary"))
                .and_then(|beside| looked_for(beside, environment));
            assert_eq!(
                binary, None,
                "the binary's location is not knowable from here"
            );
        }
        assert_eq!(
            EVERY
                .first()
                .and_then(|beside| looked_for(beside, Environment::MacOs)),
            Some("/Applications/Docker.app")
        );
        assert_eq!(
            EVERY
                .first()
                .and_then(|beside| looked_for(beside, Environment::Windows)),
            None,
            "a platform with no single place worth naming is not given a made-up one"
        );
        assert_eq!(
            EVERY
                .first()
                .and_then(|beside| looked_for(beside, Environment::LinuxNative)),
            Some("/usr/bin/dockerd")
        );
    }

    #[test]
    fn a_media_server_and_a_tunnel_are_looked_for_where_each_platform_keeps_them() {
        let jellyfin = EVERY
            .iter()
            .find(|beside| beside.what.contains("media server"));
        let tunnel = EVERY.iter().find(|beside| beside.what.contains("tunnel"));

        assert_eq!(
            jellyfin.and_then(|beside| looked_for(beside, Environment::MacOs)),
            Some("/Applications/Jellyfin.app")
        );
        assert_eq!(
            jellyfin.and_then(|beside| looked_for(beside, Environment::LinuxDesktop)),
            Some("/var/lib/jellyfin")
        );
        assert_eq!(
            jellyfin.and_then(|beside| looked_for(beside, Environment::Unsupported)),
            None
        );
        assert_eq!(
            tunnel.and_then(|beside| looked_for(beside, Environment::LinuxNative)),
            Some("/usr/sbin/tailscaled")
        );
        assert_eq!(
            tunnel.and_then(|beside| looked_for(beside, Environment::Windows)),
            None
        );
    }

    /// Found and not found are different reports, so an operator can tell "this is on
    /// your machine" from "this is what an uninstall never touches".
    #[test]
    fn what_was_found_is_reported_apart_from_what_is_merely_listed() {
        let beside = EVERY.first();
        assert_eq!(
            beside.map(|beside| against(beside, Environment::MacOs, true).found),
            Some(true)
        );
        assert_eq!(
            beside.map(|beside| against(beside, Environment::MacOs, false).found),
            Some(false)
        );
    }
}
