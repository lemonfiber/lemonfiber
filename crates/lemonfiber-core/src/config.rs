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

pub mod display;
pub mod env;
pub mod paths;
pub mod store;

use std::path::PathBuf;

use lemonfiber_manifest::Protocol;
use serde::{Deserialize, Serialize};

/// Which download protocols the operator actually has accounts for.
///
/// A form names both, because a form describes what it *does* rather than what
/// this operator has paid for. Narrowing happens afterwards, so a tunnel is
/// never started with credentials that were never supplied.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
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

/// A second, independent source asked alongside the first.
///
/// Two rather than one because the entire leak verdict is a comparison against
/// what these report: a single source that is misconfigured, cached behind a
/// proxy, or simply wrong returns a plausible address, and the check says `pass`
/// while traffic leaves in the clear. Two that disagree cannot both be trusted,
/// and saying so is the only honest answer available.
pub const SECOND_IP_ECHO: &str = "https://icanhazip.com";

/// The setting naming the IP-echo service, or switching leak detection off.
pub const IP_ECHO_KEY: &str = "LEMONFIBER_IP_ECHO";

/// The setting that switches the plain-language explanations off.
///
/// On unless it is explicitly turned off, which is the right way round: somebody
/// meeting this vocabulary for the first time does not know there is a setting to
/// look for, and somebody who finds the explanations patronising knows exactly what
/// they want to stop.
pub const EXPLANATIONS_KEY: &str = "LEMONFIBER_EXPLANATIONS";

/// The setting naming where downloads and the library are kept.
///
/// The one location the storage contract rests on: the compose driver mounts it
/// and the storage checks probe it. It is the stack's own setting rather than
/// one of lemonfiber's, so it carries no prefix.
pub const DATA_ROOT_KEY: &str = "DATA_ROOT";

/// The user id the service containers run as.
pub const PUID_KEY: &str = "PUID";

/// The group id the service containers run as.
pub const PGID_KEY: &str = "PGID";

/// The VPN provider the tunnel connects through.
///
/// Read by the port-forward check for one purpose: to name a provider's known
/// trap when port forwarding was asked for but no port arrived. It never decides
/// whether the tunnel itself works.
pub const VPN_PROVIDER_KEY: &str = "VPN_PROVIDER";

/// The setting recording whether server-side port forwarding was asked for.
///
/// Only some providers offer it, so a stack on one that does not leaves this off,
/// and the port-forward check reads that as "does not apply" rather than a fault.
pub const VPN_PORT_FORWARDING_KEY: &str = "VPN_PORT_FORWARDING";

/// The setting selecting how Jellyfin is served: in a container or on the host.
///
/// A single switch is the whole of the difference between the two modes — the
/// compose stack drops one service and the URLs change, nothing more. Absent
/// where the operator runs no media server at all.
pub const JELLYFIN_MODE_KEY: &str = "JELLYFIN_MODE";

/// The base URL of the indexer the operator gave at setup.
pub const INDEXER_URL_KEY: &str = "INDEXER_URL";

/// The indexer's API key. A secret, held here the way the stack holds its others.
pub const INDEXER_APIKEY_KEY: &str = "INDEXER_APIKEY";

/// Whether the indexer credential was proven against the live service before it
/// was kept.
///
/// Off records a credential the operator chose to proceed with unverified, so a
/// later diagnosis can point at it rather than trusting it silently.
pub const INDEXER_VALIDATED_KEY: &str = "INDEXER_VALIDATED";

/// The Usenet provider's hostname.
pub const PROVIDER_HOST_KEY: &str = "USENET_HOST";

/// The port the Usenet provider answers NNTP on.
pub const PROVIDER_PORT_KEY: &str = "USENET_PORT";

/// The Usenet account username.
pub const PROVIDER_USER_KEY: &str = "USENET_USER";

/// The Usenet account password. A secret, held the way the stack holds its others.
pub const PROVIDER_PASS_KEY: &str = "USENET_PASS";

/// Whether the Usenet connection uses TLS.
pub const PROVIDER_TLS_KEY: &str = "USENET_TLS";

/// Whether the Usenet login was proven before it was kept — off records one the
/// operator chose to proceed with unverified.
pub const PROVIDER_VALIDATED_KEY: &str = "USENET_VALIDATED";

/// The environment key holding qBittorrent's web UI password.
///
/// Seeding generates this password and records it here, because the
/// forwarded-port push authenticates to qBittorrent with it — the one credential
/// lemonfiber mints and writes rather than reads from a service.
pub const QBITTORRENT_PASSWORD_KEY: &str = "QBITTORRENT_PASSWORD";

/// The environment key holding the Jellyfin administrator password.
///
/// Jellyfin generates no key on disk and asks for an account to be created, so
/// seeding mints this password, sets it by driving Jellyfin's own first-run
/// setup, and records it here — the same shape as qBittorrent's, and the
/// credential the Seerr identity wiring reads back. The account name is
/// [`JELLYFIN_ADMIN_USER`].
pub const JELLYFIN_ADMIN_PASSWORD_KEY: &str = "JELLYFIN_ADMIN_PASSWORD";

/// The name of the Jellyfin administrator account lemonfiber creates at setup — the
/// household's own account, one source of truth for the name so the first-run driver
/// creates it, the Seerr identity wiring signs in with it, and a trace's library read
/// authenticates as it, all under the same name.
pub const JELLYFIN_ADMIN_USER: &str = "admin";

/// Every setting lemonfiber names.
///
/// Declared rather than discovered. The guard that checks nothing is displayed with its
/// value without a reason written down needs to know which settings exist, and until this
/// list it knew only the ones the embedded stack declares — which is every namespace
/// except lemonfiber's own, the one where setup collects a Usenet password and an indexer
/// key. A guard that cannot see the settings that matter most is a guard that reports on
/// somebody else's file.
///
/// Reading them back out of this module's source would answer the same question and go
/// stale the first time somebody writes a name inline, so the list is the declaration and
/// a test holds the writer to it: what [`crate::wizard::Wizard::plan`] produces, and what
/// seeding records, may only be named here.
pub const SETTINGS: &[&str] = &[
    USENET_KEY,
    TORRENT_KEY,
    IP_ECHO_KEY,
    EXPLANATIONS_KEY,
    DATA_ROOT_KEY,
    PUID_KEY,
    PGID_KEY,
    VPN_PROVIDER_KEY,
    VPN_PORT_FORWARDING_KEY,
    JELLYFIN_MODE_KEY,
    INDEXER_URL_KEY,
    INDEXER_APIKEY_KEY,
    INDEXER_VALIDATED_KEY,
    PROVIDER_HOST_KEY,
    PROVIDER_PORT_KEY,
    PROVIDER_USER_KEY,
    PROVIDER_PASS_KEY,
    PROVIDER_TLS_KEY,
    PROVIDER_VALIDATED_KEY,
    QBITTORRENT_PASSWORD_KEY,
    JELLYFIN_ADMIN_PASSWORD_KEY,
];

/// A recorded value with the whitespace and surrounding quotes a person might
/// add stripped, its case left alone — so a hand-edited `"https://IP.example"`
/// keeps its path but loses the quotes that would otherwise reach the reader.
fn unquoted(value: &str) -> String {
    value
        .trim()
        .trim_matches(|c| c == '"' || c == '\'')
        .trim()
        .to_owned()
}

/// The same, folded to lower case, so `"on"` and ` on ` both read as `on`.
fn bare(value: &str) -> String {
    unquoted(value).to_ascii_lowercase()
}

/// Whether a recorded setting reads as switched on.
///
/// Generous about spelling because this is a file people edit by hand, and a
/// setting that silently means "no" because it says `yes` rather than `on`
/// would be indistinguishable from lemonfiber ignoring it.
#[must_use]
pub fn reads_as_on(value: &str) -> bool {
    matches!(bare(value).as_str(), "on" | "true" | "yes" | "1")
}

/// Whether a recorded setting reads as switched off.
///
/// The counterpart to [`reads_as_on`] for a setting that also accepts a value:
/// an explicit off switches the feature off, where absence or an affirmative
/// would leave it on.
#[must_use]
pub fn reads_as_off(value: &str) -> bool {
    matches!(bare(value).as_str(), "off" | "false" | "no" | "0" | "")
}

/// The IP-echo service to verify egress against, or `None` where the operator
/// has switched leak detection off.
///
/// Absent or affirmative leaves the default in place; an explicit off disables
/// it, at the stated cost of losing leak detection; any other value replaces the
/// default with the operator's own endpoint — so the one third-party dependency
/// the check has is replaceable as well as disableable.
#[must_use]
pub fn ip_echo_from_env(file: &env::EnvFile) -> Vec<String> {
    let defaults = || vec![DEFAULT_IP_ECHO.to_owned(), SECOND_IP_ECHO.to_owned()];
    match file.get(IP_ECHO_KEY) {
        None => defaults(),
        Some(value) if reads_as_on(value) => defaults(),
        Some(value) if reads_as_off(value) => Vec::new(),
        // Several, comma-separated, so an operator who wants their own sources can
        // still have more than one — the point of asking two is lost if naming one
        // silently drops back to trusting a single stranger.
        Some(value) => unquoted(value)
            .split(',')
            .map(str::trim)
            .filter(|source| !source.is_empty())
            .map(str::to_owned)
            .collect(),
    }
}

/// Where the operator chose to keep downloads and media, if they have chosen.
///
/// Absent until setup writes it, and an empty value is read as absent rather
/// than as the current directory — a data root of nothing is a mistake, never an
/// intent, and probing the working directory in its place would test the wrong
/// filesystem.
#[must_use]
pub fn data_root_from_env(file: &env::EnvFile) -> Option<PathBuf> {
    file.get(DATA_ROOT_KEY)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// The indexer the operator configured, where both its URL and key are present.
///
/// Only a complete pair is an indexer to re-prove; a half-written one — a URL
/// with no key — is treated as none rather than a credential to test.
#[must_use]
pub fn indexer_from_env(file: &env::EnvFile) -> Option<Indexer> {
    let present = |key| {
        file.get(key)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    Some(Indexer {
        url: present(INDEXER_URL_KEY)?,
        key: present(INDEXER_APIKEY_KEY)?,
    })
}

/// The user and group the service containers run as, where both are configured.
///
/// Both or neither: deciding whether that user can write a directory needs the
/// pair, and one half without the other cannot answer the question, so a
/// half-configured file is treated as unconfigured rather than guessed at.
#[must_use]
pub fn service_user_from_env(file: &env::EnvFile) -> Option<(u32, u32)> {
    let uid = file.get(PUID_KEY)?.trim().parse().ok()?;
    let gid = file.get(PGID_KEY)?.trim().parse().ok()?;
    Some((uid, gid))
}

/// What the operator recorded about VPN port forwarding: whether they asked for
/// it, and which provider is meant to grant it.
///
/// Port forwarding is a per-provider capability — only some VPNs offer it — so
/// the provider name travels with the switch. The name is consulted not to decide
/// whether the tunnel works, but to name a provider's known trap when the switch
/// is on and no port arrives.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PortForward {
    /// Whether `VPN_PORT_FORWARDING` reads as switched on.
    pub enabled: bool,
    /// The provider name, lower-cased, where one is recorded.
    ///
    /// Carried even when the switch is off, but only read when it is on.
    pub provider: Option<String>,
}

/// What the operator recorded about port forwarding.
///
/// Absent or off leaves it disabled, in which case there is no forwarded port to
/// verify and the check does not apply. A blank provider is read as absent rather
/// than as a provider named the empty string.
#[must_use]
pub fn port_forward_from_env(file: &env::EnvFile) -> PortForward {
    PortForward {
        enabled: file.get(VPN_PORT_FORWARDING_KEY).is_some_and(reads_as_on),
        // `bare`, not a plain trim: the switch is de-quoted the same way, and a
        // provider left quoted would otherwise miss its known trap — the very
        // guidance the check exists to give — and read as an unknown provider.
        provider: file
            .get(VPN_PROVIDER_KEY)
            .map(bare)
            .filter(|value| !value.is_empty()),
    }
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
    pub ip_echo: Vec<String>,
    /// Where downloads and the library are kept, once setup has chosen it.
    ///
    /// Absent until then, which is why the storage checks tell an operator to
    /// run setup rather than reporting a fault about a location they never picked.
    pub data_root: Option<PathBuf>,
    /// Where the storage check records the last hardlink capability it saw, so a
    /// later run can notice the capability was lost.
    ///
    /// Absent where the surface could not find the platform's data directory, in
    /// which case the check still runs but cannot detect a change over time.
    pub storage_state: Option<PathBuf>,
    /// The user and group the service containers run as, where configured.
    ///
    /// Used to tell an operator-facing permission problem from a service-facing
    /// one: the operator may own the data root while the containers, running as
    /// this pair, cannot write it.
    pub service_user: Option<(u32, u32)>,
    /// What the operator asked for around VPN port forwarding.
    ///
    /// Disabled by default: a fresh install has no VPN configured, so the
    /// port-forward check reports that it does not apply rather than a fault.
    pub port_forward: PortForward,
    /// The indexer the operator gave at setup — its URL and key — so a diagnosis
    /// can re-prove it live. Absent where none was configured.
    pub indexer: Option<Indexer>,
    /// Whether this product explains the words it uses.
    ///
    /// On unless switched off. The words are a wall to somebody meeting them, and
    /// the operator who wants them gone is the one who knows to go and look.
    pub explanations: bool,
}

/// An indexer credential as configuration holds it: where it is, and the key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Indexer {
    /// The indexer's API base URL.
    pub url: String,
    /// The API key it authenticates with.
    pub key: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            project: crate::PRODUCT.to_owned(),
            env_file: None,
            overlays: Vec::new(),
            stack_dir: None,
            protocols: Protocols::none(),
            ip_echo: vec![DEFAULT_IP_ECHO.to_owned(), SECOND_IP_ECHO.to_owned()],
            data_root: None,
            storage_state: None,
            service_user: None,
            port_forward: PortForward::default(),
            indexer: None,
            explanations: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        env, indexer_from_env, ip_echo_from_env, Indexer, Protocol, Protocols, Settings,
        DEFAULT_IP_ECHO, SECOND_IP_ECHO,
    };

    #[test]
    fn a_fresh_install_has_no_way_to_download_yet() {
        assert!(!Protocols::none().any());
        assert_eq!(Protocols::default(), Protocols::none());
    }

    #[test]
    fn an_indexer_is_read_only_when_both_its_url_and_key_are_present() {
        // Both present: an indexer to re-prove, whitespace trimmed.
        let file = env::EnvFile::parse("INDEXER_URL= http://idx/api \nINDEXER_APIKEY=abc\n");
        assert_eq!(
            indexer_from_env(&file),
            Some(Indexer {
                url: "http://idx/api".to_owned(),
                key: "abc".to_owned(),
            })
        );

        // A URL with no key is half-written, so no indexer to test.
        let url_only = env::EnvFile::parse("INDEXER_URL=http://idx/api\n");
        assert_eq!(indexer_from_env(&url_only), None);

        // A key with no URL is nowhere to send it, so likewise none.
        let key_only = env::EnvFile::parse("INDEXER_APIKEY=abc\n");
        assert_eq!(indexer_from_env(&key_only), None);

        // An empty value counts as absent, not as a blank indexer.
        let empty = env::EnvFile::parse("INDEXER_URL=\nINDEXER_APIKEY=abc\n");
        assert_eq!(indexer_from_env(&empty), None);

        // Nothing configured at all.
        assert_eq!(indexer_from_env(&env::EnvFile::parse("")), None);
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
        // Including quoted, which a hand-edited .env commonly is.
        for on in ["on", "ON", "true", "yes", "1", " on ", "\"on\"", "'yes'"] {
            assert!(super::reads_as_on(on), "{on:?} should read as on");
        }
        for off in ["off", "false", "no", "0", "", "maybe", "\"off\"", "\"\""] {
            assert!(!super::reads_as_on(off), "{off:?} should not read as on");
            if off != "maybe" {
                assert!(super::reads_as_off(off), "{off:?} should read as off");
            }
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
        // Two sources, not one: the whole verdict is a comparison against what
        // they report, and a single stranger who is wrong makes the check say
        // `pass` while traffic leaves in the clear.
        assert_eq!(
            ip_echo_from_env(&env::EnvFile::parse("")),
            vec![DEFAULT_IP_ECHO.to_owned(), SECOND_IP_ECHO.to_owned()]
        );
        assert_eq!(
            Settings::default().ip_echo,
            vec![DEFAULT_IP_ECHO.to_owned(), SECOND_IP_ECHO.to_owned()]
        );
    }

    #[test]
    fn an_operator_can_switch_leak_detection_off() {
        for off in ["off", "OFF", "no", "false", "0", ""] {
            let file = env::EnvFile::parse(&format!("LEMONFIBER_IP_ECHO={off}\n"));
            assert!(
                ip_echo_from_env(&file).is_empty(),
                "{off:?} should disable it"
            );
        }
    }

    #[test]
    fn an_affirmative_value_leaves_the_default_in_place() {
        for on in ["on", "yes", "true", "1"] {
            let file = env::EnvFile::parse(&format!("LEMONFIBER_IP_ECHO={on}\n"));
            assert_eq!(
                ip_echo_from_env(&file),
                vec![DEFAULT_IP_ECHO.to_owned(), SECOND_IP_ECHO.to_owned()],
                "{on:?}"
            );
        }
    }

    #[test]
    fn any_other_value_replaces_the_default_endpoint() {
        let file = env::EnvFile::parse("LEMONFIBER_IP_ECHO=https://ip.example\n");
        assert_eq!(
            ip_echo_from_env(&file),
            vec!["https://ip.example".to_owned()]
        );
    }

    #[test]
    fn an_operator_can_name_several_sources_of_their_own() {
        // Naming one must not silently drop back to trusting a single stranger,
        // which is the arrangement asking two exists to avoid.
        let file =
            env::EnvFile::parse("LEMONFIBER_IP_ECHO=https://ip.example, https://other.example\n");
        assert_eq!(
            ip_echo_from_env(&file),
            vec![
                "https://ip.example".to_owned(),
                "https://other.example".to_owned()
            ]
        );
    }

    #[test]
    fn a_hand_quoted_endpoint_loses_the_quotes_but_keeps_its_case() {
        // A person editing the file by hand commonly quotes the value; the
        // quotes must not reach the container's wget, and a URL's path is
        // case-sensitive so the value is not folded like the on/off switch is.
        let file = env::EnvFile::parse("LEMONFIBER_IP_ECHO=\"https://IP.Example/Path\"\n");
        assert_eq!(
            ip_echo_from_env(&file),
            vec!["https://IP.Example/Path".to_owned()]
        );
    }

    #[test]
    fn a_data_root_is_read_when_set_and_absent_when_blank() {
        use std::path::PathBuf;

        assert_eq!(
            super::data_root_from_env(&env::EnvFile::parse("DATA_ROOT=/srv/media\n")),
            Some(PathBuf::from("/srv/media"))
        );
        // A blank or unset value is not the current directory; it is no choice
        // yet, and the storage check treats it as such.
        assert_eq!(
            super::data_root_from_env(&env::EnvFile::parse("DATA_ROOT=   \n")),
            None
        );
        assert_eq!(super::data_root_from_env(&env::EnvFile::parse("")), None);
        assert_eq!(Settings::default().data_root, None);
    }

    #[test]
    fn a_service_user_is_read_only_when_both_halves_are_present_and_numeric() {
        assert_eq!(
            super::service_user_from_env(&env::EnvFile::parse("PUID=1000\nPGID=1001\n")),
            Some((1000, 1001))
        );
        // One half without the other cannot answer the question it is for, so it
        // is treated as unconfigured rather than half-guessed.
        assert_eq!(
            super::service_user_from_env(&env::EnvFile::parse("PUID=1000\n")),
            None
        );
        assert_eq!(
            super::service_user_from_env(&env::EnvFile::parse("PUID=root\nPGID=1000\n")),
            None
        );
        assert_eq!(Settings::default().service_user, None);
    }

    #[test]
    fn port_forwarding_reads_the_switch_and_the_provider() {
        let file = env::EnvFile::parse("VPN_PROVIDER=ProtonVPN\nVPN_PORT_FORWARDING=on\n");
        let recorded = super::port_forward_from_env(&file);
        assert!(recorded.enabled);
        // The provider is lower-cased so the check can match it without caring how
        // the operator spelled it.
        assert_eq!(recorded.provider.as_deref(), Some("protonvpn"));
    }

    #[test]
    fn a_quoted_provider_is_de_quoted_like_the_switch() {
        // A hand-edited .env commonly quotes values. The provider must be stripped
        // the same way the switch is, or a quoted name misses its known trap and
        // reads as an unknown provider.
        let file = env::EnvFile::parse("VPN_PROVIDER=\"protonvpn\"\nVPN_PORT_FORWARDING=\"on\"\n");
        let recorded = super::port_forward_from_env(&file);
        assert!(recorded.enabled);
        assert_eq!(recorded.provider.as_deref(), Some("protonvpn"));
    }

    #[test]
    fn port_forwarding_is_off_by_default_and_a_blank_provider_is_absent() {
        // A fresh install has no VPN configured, so nothing to verify a port for.
        assert!(!super::port_forward_from_env(&env::EnvFile::parse("")).enabled);
        assert_eq!(
            Settings::default().port_forward,
            super::PortForward::default()
        );

        // A named-but-empty provider is no provider, not one called "".
        let blank = env::EnvFile::parse("VPN_PROVIDER=   \nVPN_PORT_FORWARDING=off\n");
        let recorded = super::port_forward_from_env(&blank);
        assert!(!recorded.enabled);
        assert_eq!(recorded.provider, None);
    }
}
