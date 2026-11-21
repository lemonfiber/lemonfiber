//! What setup decided, and what revisiting each decision costs.
//!
//! Setup asks its questions at the moment the operator knows least about the system,
//! so some of the answers will be wrong. Without a first-class way to revise one, the
//! ways out are editing files nobody was meant to see, or tearing the stack down and
//! starting again — which risks the library. An unchangeable decision is a trap.
//!
//! So every answer setup writes is listed here with what changing it costs and what
//! changing it affects. The cost is the part that has to arrive *before* the write:
//! moving the data location without re-pointing the curators leaves a library that points
//! at nothing, and an operator told that afterwards has already lost the thing the
//! telling was for.
//!
//! Beside the catalogue is the shape of one revision: what a setting holds, what it
//! would hold, what that costs, and whether the proposal cleared everything standing
//! between it and the file. A change is a thing that can be read and decided on before
//! it happens, rather than a value that has already been written.
//!
//! And beside that, what the revision comes to on the machine it is made on: what
//! adding or dropping a way of downloading opens and keeps, what moving the data
//! location does to the library paths the services already hold, and whether the file
//! was edited by hand since lemonfiber last wrote to it. Each is decided here from
//! what somebody else went and read.
//!
//! Nothing here reads a file or reaches a service. It is the catalogue and the shape of
//! a proposal against it, and the surfaces that write settings consult both.

use crate::config::{
    DATA_ROOT_KEY, FRONT_DOOR_KEY, INDEXER_APIKEY_KEY, INDEXER_URL_KEY, MEDIA_SERVER_MODE_KEY,
    PGID_KEY, PROVIDER_HOST_KEY, PROVIDER_PASS_KEY, PROVIDER_PORT_KEY, PROVIDER_TLS_KEY,
    PROVIDER_USER_KEY, PUID_KEY, TORRENT_KEY, USENET_KEY, VPN_KEY, VPN_PROVIDER_KEY, ZONE_KEY,
};

// Reached by their own names rather than flattened up here. Three questions about
// one change read better spelled as the questions they are — `protocols::opened`,
// `relocating::moving`, `edits::standing` — than as a dozen verbs in one namespace
// where nothing would say which of them belong together.
pub mod edits;
mod findings;
pub mod protocols;
pub mod relocating;
mod review;

pub use findings::{Active, Edited, Findings, LibraryPath, Opening};
pub use review::{Change, Consent, Review, Stance};

/// What changing a decision costs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Cost {
    /// Applied with a restart of the services it affects, and nothing else moves.
    Cheap,
    /// Said and confirmed before it is applied: it may move data, invalidate library
    /// paths, or take a service away.
    Consequential,
}

/// The setting an answer setup writes is held in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionKey {
    /// Where the library and every download live.
    DataRoot,
    /// Whether the media server runs as a container or natively.
    MediaServerMode,
    /// Whether the stack downloads over Usenet.
    Usenet,
    /// Whether the stack torrents.
    Torrent,
    /// Whether a VPN carries the torrents.
    Vpn,
    /// The user the containers run as.
    Puid,
    /// The group the containers run as.
    Pgid,
    /// The provider the VPN reaches.
    VpnProvider,
    /// Where searches are sent.
    IndexerUrl,
    /// The key searches are sent with.
    IndexerApikey,
    /// The Usenet provider articles are fetched from.
    ProviderHost,
    /// The port the Usenet provider is reached on.
    ProviderPort,
    /// The account articles are fetched with.
    ProviderUser,
    /// The password that account is reached with.
    ProviderPass,
    /// Whether the Usenet provider is reached over TLS.
    ProviderTls,
    /// The service the front door names.
    FrontDoor,
    /// The house's time zone.
    Zone,
    /// The house's language and region.
    Locale,
}

impl DecisionKey {
    /// The name the setting is written under.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DataRoot => DATA_ROOT_KEY,
            Self::MediaServerMode => MEDIA_SERVER_MODE_KEY,
            Self::Usenet => USENET_KEY,
            Self::Torrent => TORRENT_KEY,
            Self::Vpn => VPN_KEY,
            Self::Puid => PUID_KEY,
            Self::Pgid => PGID_KEY,
            Self::VpnProvider => VPN_PROVIDER_KEY,
            Self::IndexerUrl => INDEXER_URL_KEY,
            Self::IndexerApikey => INDEXER_APIKEY_KEY,
            Self::ProviderHost => PROVIDER_HOST_KEY,
            Self::ProviderPort => PROVIDER_PORT_KEY,
            Self::ProviderUser => PROVIDER_USER_KEY,
            Self::ProviderPass => PROVIDER_PASS_KEY,
            Self::ProviderTls => PROVIDER_TLS_KEY,
            Self::FrontDoor => FRONT_DOOR_KEY,
            Self::Zone => ZONE_KEY,
            Self::Locale => crate::locale::SETTING,
        }
    }
}

impl serde::Serialize for DecisionKey {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl schemars::JsonSchema for DecisionKey {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "DecisionKey".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        let keys: Vec<serde_json::Value> = DECISIONS
            .iter()
            .map(|decision| serde_json::json!({ "type": "string", "const": decision.key.as_str() }))
            .collect();
        schemars::json_schema!({
            "description": "The setting an answer setup writes is held in.",
            "oneOf": keys,
        })
    }
}

/// One answer setup wrote, as it can be revisited afterwards.
pub struct Decision {
    /// The setting the answer is held in.
    pub key: DecisionKey,
    /// What changing it costs.
    pub cost: Cost,
    /// What changing it affects, in the operator's terms — stated before the write,
    /// because after it the damage is already done.
    pub affects: &'static str,
}

/// Every answer setup writes, with what revising it costs.
///
/// The three questions setup asks that have no configuration home — the household,
/// notifications and autostart — are applied by their own features. They are
/// deliberately absent rather than duplicated into a second surface that could disagree
/// with the first.
///
/// The household and autostart are revised through their own commands. The notification
/// appetite is not: setup writes it and nothing changes it afterwards, which is a
/// decision made once and then unchangeable, which is the trap this catalogue exists to
/// close. It is listed as owed rather than quietly left out.
pub(crate) const DECISIONS: [Decision; 18] = [
    Decision {
        key: DecisionKey::DataRoot,
        cost: Cost::Consequential,
        affects: "where every download and every library file lives. Each curator holds absolute paths to its root folders, so moving this without re-pointing them leaves a library that points at nothing",
    },
    Decision {
        key: DecisionKey::MediaServerMode,
        cost: Cost::Consequential,
        affects: "whether the media server runs as a container or natively. Its configuration and its library paths differ between the two and have to be carried across",
    },
    Decision {
        key: DecisionKey::Usenet,
        cost: Cost::Consequential,
        affects: "whether Usenet runs at all. Turning it on opens the prerequisites and the credentials Usenet needs and nothing else; turning it off stops and removes its services and disables the forms that required them, and keeps everything already downloaded",
    },
    Decision {
        key: DecisionKey::Torrent,
        cost: Cost::Consequential,
        affects: "whether torrents run at all, and with them the VPN that carries them. Turning it off stops and removes the download client and the tunnel, and keeps everything already downloaded",
    },
    Decision {
        key: DecisionKey::Vpn,
        cost: Cost::Consequential,
        affects: "whether a VPN carries the torrents. Turning it off stops the tunnel and runs the torrent client on its own network, where every peer sees this connection's own address; turning it on puts the client back inside the tunnel",
    },
    Decision {
        key: DecisionKey::Puid,
        cost: Cost::Consequential,
        affects: "which user the containers run as. Files already written stay owned by the previous one, so a change here can leave the stack unable to read its own library",
    },
    Decision {
        key: DecisionKey::Pgid,
        cost: Cost::Consequential,
        affects: "which group the containers run as. Files already written stay owned by the previous one, so a change here can leave the stack unable to read its own library",
    },
    Decision {
        key: DecisionKey::VpnProvider,
        cost: Cost::Consequential,
        affects: "which provider carries the torrents. The tunnel is rebuilt, and torrents do not move until it is up again",
    },
    Decision {
        key: DecisionKey::IndexerUrl,
        cost: Cost::Cheap,
        affects: "where searches are sent. It is proven against the live indexer before the previous one is discarded",
    },
    Decision {
        key: DecisionKey::IndexerApikey,
        cost: Cost::Cheap,
        affects: "the key searches are sent with. It is proven against the live indexer before the previous one is discarded",
    },
    Decision {
        key: DecisionKey::ProviderHost,
        cost: Cost::Cheap,
        affects: "which Usenet provider articles are fetched from. The login is proven before the previous one is discarded",
    },
    Decision {
        key: DecisionKey::ProviderPort,
        cost: Cost::Cheap,
        affects: "the port the provider is reached on. The login is proven before the previous one is discarded",
    },
    Decision {
        key: DecisionKey::ProviderUser,
        cost: Cost::Cheap,
        affects: "the account articles are fetched with. The login is proven before the previous one is discarded",
    },
    Decision {
        key: DecisionKey::ProviderPass,
        cost: Cost::Cheap,
        affects: "the password the account is reached with. The login is proven before the previous one is discarded",
    },
    Decision {
        key: DecisionKey::ProviderTls,
        cost: Cost::Cheap,
        affects: "whether the provider is reached over TLS. A password is never sent over a plaintext connection, so turning this off leaves the login unproven rather than proving it in the clear",
    },
    Decision {
        key: DecisionKey::FrontDoor,
        cost: Cost::Cheap,
        affects: "which service the front door names",
    },
    Decision {
        key: DecisionKey::Zone,
        cost: Cost::Cheap,
        affects: "the zone every service keeps its clock in, and the one a service presenting the house to the outside says it is in. Each service reads it again when it restarts",
    },
    Decision {
        key: DecisionKey::Locale,
        cost: Cost::Cheap,
        affects: "the language and region a service presenting the house to the outside says it has. Each service that reads it takes it again when it restarts",
    },
];

/// What setup decided about `key`, where it decided anything.
#[must_use]
pub fn decision(key: &str) -> Option<&'static Decision> {
    DECISIONS
        .iter()
        .find(|decision| decision.key.as_str() == key)
}

/// What changing `key` costs.
///
/// A setting nobody asked about during setup is not a decision this catalogue speaks
/// for, and is cheap: the operator reaching for it by name has gone looking for it, and
/// inventing a cost for a setting whose consequences nobody worked out would teach them
/// to dismiss the ones that mean something.
#[must_use]
pub fn cost(key: &str) -> Cost {
    decision(key).map_or(Cost::Cheap, |decision| decision.cost)
}

/// Whether changing `key` is consequential enough to be confirmed first.
#[must_use]
pub fn consequential(key: &str) -> bool {
    cost(key) == Cost::Consequential
}

#[cfg(test)]
mod tests;
