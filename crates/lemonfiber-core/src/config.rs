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

/// The IP-echo service the leak check asks each container for its public
/// address.
///
/// A plain endpoint that answers with the caller's address and nothing else, so
/// the check runs `wget` against it from inside the containers rather than
/// lemonfiber reaching the network on their behalf.
pub const DEFAULT_IP_ECHO: &str = "https://ifconfig.me";

/// The setting naming the IP-echo service, or switching leak detection off.
pub const IP_ECHO_KEY: &str = "LEMONFIBER_IP_ECHO";

/// The single data mount every service shares, beneath which downloads and media
/// are subdirectories on one filesystem.
///
/// It is the stack's own variable rather than one under lemonfiber's prefix,
/// because Compose expands it directly into every service's one bind mount. The
/// storage probe tests exactly this location, since it is the volume imports
/// hardlink onto.
pub const DATA_ROOT_KEY: &str = "DATA_ROOT";

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

/// Whether a recorded setting reads as switched off.
///
/// The counterpart to [`reads_as_on`] for a setting that also accepts a value:
/// an explicit off switches the feature off, where absence or an affirmative
/// would leave it on.
#[must_use]
pub fn reads_as_off(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "off" | "false" | "no" | "0" | ""
    )
}

/// The IP-echo service to verify egress against, or `None` where the operator
/// has switched leak detection off.
///
/// Absent or affirmative leaves the default in place; an explicit off disables
/// it, at the stated cost of losing leak detection; any other value replaces the
/// default with the operator's own endpoint — so the one third-party dependency
/// the check has is replaceable as well as disableable.
#[must_use]
pub fn ip_echo_from_env(file: &env::EnvFile) -> Option<String> {
    match file.get(IP_ECHO_KEY) {
        None => Some(DEFAULT_IP_ECHO.to_owned()),
        Some(value) if reads_as_on(value) => Some(DEFAULT_IP_ECHO.to_owned()),
        Some(value) if reads_as_off(value) => None,
        Some(value) => Some(value.trim().to_owned()),
    }
}

/// The data root the operator chose, where one has been recorded.
///
/// A blank value is treated as unset rather than as the current directory: an
/// empty `DATA_ROOT=` in a half-finished file is an operator who has not chosen
/// yet, and probing the working directory would test the wrong volume and report
/// a capability the real data root may not have.
#[must_use]
pub fn data_root_from_env(file: &env::EnvFile) -> Option<PathBuf> {
    file.get(DATA_ROOT_KEY)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
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

/// What the operator chose: enough for the compose driver to know what runs,
/// and the answers other subsystems act on.
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
    /// The IP-echo service the VPN leak check compares egress against, or `None`
    /// where the operator has switched leak detection off.
    ///
    /// On by default, because the failure it catches is the one whose
    /// consequences reach outside the machine.
    pub ip_echo: Option<String>,
    /// The volume downloads and media share, which the storage probe tests.
    ///
    /// Absent until setup has chosen a location, which is why the storage check
    /// reports itself skipped rather than probing the wrong directory.
    pub data_root: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            project: crate::PRODUCT.to_owned(),
            env_file: None,
            overlays: Vec::new(),
            stack_dir: None,
            protocols: Protocols::none(),
            ip_echo: Some(DEFAULT_IP_ECHO.to_owned()),
            data_root: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        data_root_from_env, env, ip_echo_from_env, Protocol, Protocols, Settings, DEFAULT_IP_ECHO,
    };

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

    #[test]
    fn leak_detection_is_on_by_default() {
        // The failure it catches reaches outside the machine, so a fresh install
        // is protected without the operator having to ask.
        assert_eq!(
            ip_echo_from_env(&env::EnvFile::parse("")).as_deref(),
            Some(DEFAULT_IP_ECHO)
        );
        assert_eq!(
            Settings::default().ip_echo.as_deref(),
            Some(DEFAULT_IP_ECHO)
        );
    }

    #[test]
    fn an_operator_can_switch_leak_detection_off() {
        for off in ["off", "OFF", "no", "false", "0", ""] {
            let file = env::EnvFile::parse(&format!("LEMONFIBER_IP_ECHO={off}\n"));
            assert_eq!(ip_echo_from_env(&file), None, "{off:?} should disable it");
        }
    }

    #[test]
    fn an_affirmative_value_leaves_the_default_in_place() {
        for on in ["on", "yes", "true", "1"] {
            let file = env::EnvFile::parse(&format!("LEMONFIBER_IP_ECHO={on}\n"));
            assert_eq!(
                ip_echo_from_env(&file).as_deref(),
                Some(DEFAULT_IP_ECHO),
                "{on:?}"
            );
        }
    }

    #[test]
    fn any_other_value_replaces_the_default_endpoint() {
        let file = env::EnvFile::parse("LEMONFIBER_IP_ECHO=https://ip.example\n");
        assert_eq!(
            ip_echo_from_env(&file).as_deref(),
            Some("https://ip.example")
        );
    }

    #[test]
    fn a_recorded_data_root_is_the_location_to_probe() {
        let file = env::EnvFile::parse("DATA_ROOT=/srv/media\n");
        assert_eq!(data_root_from_env(&file), Some(PathBuf::from("/srv/media")));
        assert_eq!(Settings::default().data_root, None);
    }

    #[test]
    fn an_absent_or_blank_data_root_is_treated_as_unchosen() {
        // A half-finished file with an empty value is an operator who has not
        // chosen yet, not one who chose the working directory.
        assert_eq!(data_root_from_env(&env::EnvFile::parse("")), None);
        for blank in ["DATA_ROOT=\n", "DATA_ROOT=   \n"] {
            assert_eq!(
                data_root_from_env(&env::EnvFile::parse(blank)),
                None,
                "{blank:?}"
            );
        }
    }
}
