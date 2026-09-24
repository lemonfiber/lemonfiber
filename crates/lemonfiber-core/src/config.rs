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
    unmanaged_from_env, PortForward,
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
    /// The plugins this machine has installed, by id.
    ///
    /// Ids rather than paths, because where a plugin's document lives is a function
    /// of the directory Compose is pointed at — and that is settled per invocation,
    /// by whether the stack is the embedded one or one the operator named. A resolved
    /// path here would be right for one of those and quietly wrong for the other.
    pub plugins: Vec<String>,
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
    /// The areas the operator declared unmanaged, each with the reason they gave.
    ///
    /// Empty on every machine where nobody has said anything, which is most of them.
    /// What a name covers and what a declaration stops is in [`crate::unmanaged`].
    pub unmanaged: Vec<(String, String)>,
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
            plugins: Vec::new(),
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
            unmanaged: Vec::new(),
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
mod tests;
