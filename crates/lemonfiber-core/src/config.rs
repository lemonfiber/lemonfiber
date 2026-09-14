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
mod keys;
pub mod paths;
pub mod reaching;
mod reading;
pub mod store;

// Taken whole rather than named one by one, the way the readers below are. Every
// name in that module is a setting this one owns and passes on unchanged, so a list
// here would be the same list written a second time — and of two copies of a list,
// the one that goes stale is the one nobody reads.
pub use keys::*;

// Re-exported rather than reached for through the module they now live in: what a
// recorded value comes to is this module's business, and moving the reading of one
// would otherwise be a change at every call site that asks.
pub use reading::{
    data_root_from_env, exposed_from_env, front_door_from_env, household_host_from_env,
    indexer_from_env, ip_echo_from_env, overlay_from_env, port_forward_from_env, project_from_env,
    provider_host_from_env, quiet_from_env, reads_as_off, reads_as_on, service_user_from_env,
    PortForward,
};

use std::path::PathBuf;

use lemonfiber_manifest::Protocol;
use serde::{Deserialize, Serialize};

use crate::ports::docker::Target;

pub use reaching::{
    offline, Reaching, OFFLINE_KEY, REACH_GUIDES_KEY, REACH_HOUSEHOLD_KEY, REACH_INDEXER_KEY,
    REACH_REGISTRY_KEY, REACH_UPDATES_KEY, REACH_USENET_KEY, SWITCHES,
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
    /// The hours the operator does not want waking for, where they have said.
    pub quiet: Option<crate::alert::Quiet>,
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
    /// Whether a start at a login may happen while this machine is on its battery.
    ///
    /// Off unless switched on. Read only by the run that a login starts; typing
    /// `lemonfiber up` yourself is you deciding, and nothing here second-guesses it.
    pub autostart_on_battery: bool,
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
    /// Where this binary is on this machine, which a hosted service has to name.
    ///
    /// Absent where the platform would not say, which is why hosting refuses by
    /// name rather than installing a service against a guessed path — a service
    /// naming the wrong program is one that fails at every login and says nothing.
    pub program: Option<PathBuf>,
    /// The directory a hosted command's words are written into.
    ///
    /// A hosted run has no terminal to say them in. Absent where the surface
    /// could not find the platform's data directory, which is the same absence
    /// every other location here handles.
    pub hosted: Option<PathBuf>,
    /// This operator's home directory, as the platform reports it.
    ///
    /// Read for one thing only: the records the tools that install programs leave
    /// beneath it, which is how the copy of lemonfiber that is running can say which
    /// of them put it there. Absent where the platform would not say, which reads as
    /// a machine with no such record rather than as a failure.
    pub home: Option<PathBuf>,
    /// Which container engine this run operates, and how it came to be that one.
    ///
    /// Resolved once at the edge from the environment and Docker's own records, and
    /// held here because this is what both halves of a run read. The Engine API
    /// client is built from it and the Compose invocation is given it as `--host`,
    /// so there is no arrangement of settings under which the reads and the writes
    /// reach different machines — which is what they did while the client resolved
    /// its own endpoint and Compose inherited the environment.
    ///
    /// This machine's own daemon unless something said otherwise, which is the
    /// ordinary case and the one that must stay silent.
    pub docker: Target,
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
            quiet: None,
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
            autostart_on_battery: false,
            reaching: Reaching::default(),
            provider_host: None,
            program: None,
            hosted: None,
            home: None,
            docker: Target::local(),
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
