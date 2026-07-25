//! What the operator chose, and where it is kept.
//!
//! Reading and writing the environment file preserves comments and ordering,
//! because it is a file an operator edits by hand and a rewrite that reorders it
//! destroys their annotations. Configuration written by a newer build is refused
//! rather than modified — silently downgrading a config file is how a
//! downgrade-to-test becomes an unrecoverable state.
//!
//! The file itself arrives with the setup wizard. What is settled here is the
//! shape the compose driver reads: enough to decide what runs, and nothing about
//! how it is stored.

pub mod env;
pub mod paths;
pub mod store;

use std::path::PathBuf;

use lemonfiber_manifest::Protocol;
use serde::Serialize;

/// Which download protocols the operator actually has accounts for.
///
/// A form names both, because a form describes what it *does* rather than what
/// this operator has paid for. Narrowing happens afterwards, so a tunnel is
/// never started with credentials that were never supplied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct Protocols {
    /// A Usenet provider is configured.
    pub usenet: bool,
    /// A VPN and torrent client are configured.
    pub torrent: bool,
}

/// The setting recording that a Usenet provider is configured.
///
/// lemonfiber's own answers live in the same file as the stack's settings,
/// under a prefix of its own. Compose is handed the file and will pass these to
/// containers that ask for them, which nothing does — the alternative, a second
/// configuration file, would mean an operator keeping two things in step.
pub const USENET_KEY: &str = "LEMONFIBER_USENET";

/// The setting recording that a VPN and torrent client are configured.
pub const TORRENT_KEY: &str = "LEMONFIBER_TORRENT";

/// Whether a recorded setting reads as switched on.
///
/// Generous about spelling because this is a file people edit by hand, and a
/// setting that silently means "no" because it says `yes` rather than `on`
/// would be indistinguishable from lemonfiber ignoring it.
#[must_use]
pub fn reads_as_on(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "on" | "true" | "yes" | "1"
    )
}

impl Protocols {
    /// Neither protocol configured — what a fresh install looks like.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            usenet: false,
            torrent: false,
        }
    }

    /// Both protocols configured.
    #[must_use]
    pub const fn both() -> Self {
        Self {
            usenet: true,
            torrent: true,
        }
    }

    /// Whether there is at least one configured way to download.
    #[must_use]
    pub const fn any(self) -> bool {
        self.usenet || self.torrent
    }

    /// What the operator has recorded about their providers.
    #[must_use]
    pub fn from_env(file: &env::EnvFile) -> Self {
        Self {
            usenet: file.get(USENET_KEY).is_some_and(reads_as_on),
            torrent: file.get(TORRENT_KEY).is_some_and(reads_as_on),
        }
    }

    /// Whether the provider a profile declared is one the operator configured.
    #[must_use]
    pub const fn has(self, protocol: Protocol) -> bool {
        match protocol {
            Protocol::Usenet => self.usenet,
            Protocol::Torrent => self.torrent,
        }
    }
}

/// The settings the compose driver reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// The Compose project name, which is also how containers are correlated
    /// back to the services that declared them.
    pub project: String,
    /// The environment file handed to Compose, where one has been written.
    pub env_file: Option<PathBuf>,
    /// Compose files layered over the stack's own, such as a storage overlay.
    pub overlays: Vec<PathBuf>,
    /// Where an embedded stack is written so Compose can read it.
    ///
    /// Absent until setup has chosen a location, which is why an operator who
    /// has not run setup is told to rather than shown a path error.
    pub stack_dir: Option<PathBuf>,
    /// Which download protocols are configured.
    pub protocols: Protocols,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            project: crate::PRODUCT.to_owned(),
            env_file: None,
            overlays: Vec::new(),
            stack_dir: None,
            protocols: Protocols::none(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{env, Protocol, Protocols, Settings};

    #[test]
    fn a_fresh_install_has_no_way_to_download_yet() {
        assert!(!Protocols::none().any());
        assert_eq!(Protocols::default(), Protocols::none());
    }

    #[test]
    fn either_protocol_alone_counts() {
        let usenet = Protocols {
            usenet: true,
            torrent: false,
        };
        let torrent = Protocols {
            usenet: false,
            torrent: true,
        };
        assert!(usenet.any());
        assert!(torrent.any());
        assert!(Protocols::both().any());
    }

    #[test]
    fn each_protocol_answers_for_itself() {
        let usenet_only = Protocols {
            usenet: true,
            torrent: false,
        };
        assert!(usenet_only.has(Protocol::Usenet));
        assert!(!usenet_only.has(Protocol::Torrent));
        assert!(Protocols::both().has(Protocol::Torrent));
        assert!(!Protocols::none().has(Protocol::Usenet));
    }

    #[test]
    fn a_provider_is_configured_only_when_it_says_so() {
        let file = env::EnvFile::parse("LEMONFIBER_USENET=on\nLEMONFIBER_TORRENT=off\n");
        assert_eq!(
            Protocols::from_env(&file),
            Protocols {
                usenet: true,
                torrent: false
            }
        );
    }

    #[test]
    fn nothing_recorded_means_nothing_configured() {
        assert_eq!(
            Protocols::from_env(&env::EnvFile::parse("")),
            Protocols::none()
        );
    }

    #[test]
    fn a_setting_may_be_spelled_the_ways_people_spell_it() {
        for on in ["on", "ON", "true", "yes", "1", " on "] {
            assert!(super::reads_as_on(on), "{on:?} should read as on");
        }
        for off in ["off", "false", "no", "0", "", "maybe"] {
            assert!(!super::reads_as_on(off), "{off:?} should not read as on");
        }
    }

    #[test]
    fn the_project_name_defaults_to_the_product() {
        let settings = Settings::default();
        assert_eq!(settings.project, "lemonfiber");
        assert_eq!(settings.env_file, None);
        assert_eq!(settings.stack_dir, None);
        assert!(settings.overlays.is_empty());
    }
}
