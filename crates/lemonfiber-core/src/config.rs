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
pub mod reaching;
mod reading;
pub mod store;

// Re-exported rather than reached for through the module they now live in: what a
// recorded value comes to is this module's business, and moving the reading of one
// would otherwise be a change at every call site that asks.
pub use reading::{
    data_root_from_env, exposed_from_env, front_door_from_env, household_host_from_env,
    indexer_from_env, ip_echo_from_env, port_forward_from_env, provider_host_from_env,
    reads_as_off, reads_as_on, service_user_from_env, PortForward,
};

use std::path::PathBuf;

use lemonfiber_manifest::Protocol;
use serde::{Deserialize, Serialize};

pub use reaching::{
    offline, Reaching, OFFLINE_KEY, REACH_GUIDES_KEY, REACH_INDEXER_KEY, REACH_REGISTRY_KEY,
    REACH_USENET_KEY, SWITCHES,
};

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

/// The admin services the operator has said out loud they meant to expose.
///
/// The diagnosis offers to stop reporting an exposed admin surface "if you meant
/// it", and until this existed there was nowhere to say so — a remedy offering an
/// action nobody could take. This is where it is taken.
///
/// A name and a reason, because a name on its own records that somebody clicked
/// past a warning and nothing about why. The reason is what a person reading this
/// file in a year, or reading a support bundle, is actually served by, and it is the
/// same standard the displayed-settings register is held to.
///
/// `sonarr=it is behind the reverse proxy I already run,radarr=the same`
pub const EXPOSED_KEY: &str = "LEMONFIBER_EXPOSED";

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

/// The address the household's own links are pointed at.
///
/// The stack's, and the one thing in it that says where this machine is reached
/// from another device in the house. It ships pointed at this machine and nowhere
/// else, which is the right default for a machine nobody has told where it is and
/// the wrong address to hand anybody.
pub const HOUSEHOLD_HOST_KEY: &str = "HOMEPAGE_VAR_LAN_HOST";

/// The service the operator chose to send the household to, by the id the stack
/// declares it under.
///
/// lemonfiber's own answer rather than the stack's, so it carries lemonfiber's
/// prefix. What a name here may be is [`crate::door`]'s to decide and not this
/// module's: the question is which tier a service is published on, and the answer
/// belongs where the rest of that reasoning already lives.
pub const FRONT_DOOR_KEY: &str = "LEMONFIBER_FRONT_DOOR";

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

/// The environment key holding the listening server's first-account password.
///
/// The same shape as Jellyfin's and for the same reason: the service starts with no
/// account and writes no key, so lemonfiber mints this, creates the account with it,
/// and keeps it. The token its dashboard panel uses is derived from this on demand
/// rather than recorded beside it — the service hands back the same one every
/// sign-in, so a second record would be a second copy of the same secret.
pub const AUDIOBOOKSHELF_PASSWORD_KEY: &str = "AUDIOBOOKSHELF_PASSWORD";

/// The name of the listening server's first account.
pub const AUDIOBOOKSHELF_USER: &str = "admin";

/// The name of the Jellyfin administrator account lemonfiber creates at setup — the
/// household's own account, one source of truth for the name so the first-run driver
/// creates it, the Seerr identity wiring signs in with it, and a trace's library read
/// authenticates as it, all under the same name.
pub const JELLYFIN_ADMIN_USER: &str = "admin";

/// The account name qBittorrent's web UI is reached under.
///
/// One source of truth for the name, so the client that logs in, the download-client
/// registration that hands it to an \*arr, and the dashboard's own widget all present
/// the same one. Separate from Jellyfin's although both spell it `admin`: they are two
/// services, and either may change without the other.
pub const QBITTORRENT_USER: &str = "admin";

/// The environment key holding the account name qBittorrent is reached under.
///
/// The dashboard reads both halves of the credential from the environment and has no
/// default for this one, so a name that is never written leaves its widget unable to
/// authenticate.
pub const QBITTORRENT_USERNAME_KEY: &str = "QBITTORRENT_USERNAME";

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
/// a test holds the writer to it: what [`crate::wizard::Wizard::plan`] produces may only
/// be named here.
///
/// Seeding is not held to this list. What it records is partly computed — a key is
/// published under the id of the service it was read from — so the names cannot be
/// declared ahead of knowing the stack.
pub const SETTINGS: &[&str] = &[
    USENET_KEY,
    TORRENT_KEY,
    IP_ECHO_KEY,
    OFFLINE_KEY,
    REACH_REGISTRY_KEY,
    REACH_GUIDES_KEY,
    REACH_INDEXER_KEY,
    REACH_USENET_KEY,
    EXPLANATIONS_KEY,
    EXPOSED_KEY,
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
    AUDIOBOOKSHELF_PASSWORD_KEY,
    FRONT_DOOR_KEY,
];

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
    /// Where the password the web surface asks for is kept, as the verifier that
    /// proves it.
    ///
    /// Absent where the surface could not find the platform's configuration
    /// directory, which is the same absence every other location here handles — and
    /// which reads as no authentication configured, because a credential nothing can
    /// find is a credential nothing can check.
    pub admission: Option<PathBuf>,
    /// The address the operator recorded for the household's own links.
    ///
    /// Where the front door is reached from another device in the house, on a
    /// machine whose own name is not published on the network. Absent until they
    /// record one, which is the state a fresh install is in.
    pub household_host: Option<String>,
    /// The admin services the operator wrote down as deliberately exposed, each
    /// with the reason they gave.
    pub exposed: Vec<(String, String)>,
    /// The service the operator named as the front door, where they named one.
    ///
    /// Absent until they do, which is the state a fresh install is in and the one
    /// where the door is worked out from what the stack declares. A name here is
    /// what they asked for rather than what they get: one this stack does not
    /// publish to the household is refused and said, not obeyed.
    pub front_door: Option<String>,
    /// Whether this product explains the words it uses.
    ///
    /// On unless switched off. The words are a wall to somebody meeting them, and
    /// the operator who wants them gone is the one who knows to go and look.
    pub explanations: bool,
    /// Which requests lemonfiber may make on its own account.
    ///
    /// Every one allowed unless the operator said otherwise, and what each of them
    /// costs to refuse is stated where the list is built rather than here.
    pub reaching: Reaching,
    /// The Usenet provider's hostname, where one was configured.
    ///
    /// The host alone and never the account beside it: what reads this is the list
    /// of where this machine's requests go, and where is a hostname.
    pub provider_host: Option<String>,
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
            admission: None,
            household_host: None,
            exposed: Vec::new(),
            front_door: None,
            explanations: true,
            reaching: Reaching::default(),
            provider_host: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        env, front_door_from_env, household_host_from_env, indexer_from_env, ip_echo_from_env,
        provider_host_from_env, Indexer, Protocol, Protocols, Settings, DEFAULT_IP_ECHO,
        OFFLINE_KEY, SECOND_IP_ECHO,
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
    fn the_household_address_is_read_and_a_blank_one_is_absent() {
        // A blank is a mistake, never an intent — and an address of nothing is the
        // one thing worse to hand somebody than no address at all.
        let written = env::EnvFile::parse("HOMEPAGE_VAR_LAN_HOST= 192.168.1.10 \n");
        assert_eq!(
            household_host_from_env(&written),
            Some("192.168.1.10".to_owned())
        );
        let blank = env::EnvFile::parse("HOMEPAGE_VAR_LAN_HOST=\n");
        assert_eq!(household_host_from_env(&blank), None);
        assert_eq!(household_host_from_env(&env::EnvFile::parse("")), None);
    }

    #[test]
    fn the_named_front_door_is_read_and_a_blank_one_is_absent() {
        // The same reading and the same reason: a blank is a mistake, never an
        // intent, and a door named nothing would refuse on every run.
        let written = env::EnvFile::parse("LEMONFIBER_FRONT_DOOR= jellyfin \n");
        assert_eq!(front_door_from_env(&written), Some("jellyfin".to_owned()));
        let blank = env::EnvFile::parse("LEMONFIBER_FRONT_DOOR=\n");
        assert_eq!(front_door_from_env(&blank), None);
        assert_eq!(front_door_from_env(&env::EnvFile::parse("")), None);
        assert_eq!(Settings::default().front_door, None);
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

    /// The one setting here that names a thing as well as answering yes or no, so
    /// the blanket switch has to be read where it is read rather than beside the
    /// four that only answer.
    #[test]
    fn the_blanket_switch_stops_the_leak_check_even_where_a_source_is_named() {
        let file = env::EnvFile::parse(&format!(
            "{OFFLINE_KEY}=on\nLEMONFIBER_IP_ECHO=https://ip.example\n"
        ));
        assert!(ip_echo_from_env(&file).is_empty());
    }

    /// Where the requests that leave this machine are listed, the Usenet provider is
    /// named by its host — so the host is read back out of the file, and the account
    /// beside it never is.
    #[test]
    fn a_usenet_host_is_read_back_and_a_blank_one_is_absent() {
        assert_eq!(
            provider_host_from_env(&env::EnvFile::parse("USENET_HOST= news.example.net \n")),
            Some("news.example.net".to_owned())
        );
        for blank in ["USENET_HOST=\n", "USENET_HOST=   \n", "PUID=1000\n"] {
            assert_eq!(
                provider_host_from_env(&env::EnvFile::parse(blank)),
                None,
                "{blank:?}"
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
