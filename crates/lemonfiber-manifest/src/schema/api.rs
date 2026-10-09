//! How lemonfiber talks to a service: the adapter it names and where its credential is.

use serde::{Deserialize, Serialize};

/// How lemonfiber talks to a service when seeding.
///
/// The same shape on a plugin's service as on the stack's own, which is what lets a
/// plugin name one of these adapters rather than supply one of its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Api {
    /// Selects the client implementation.
    pub kind: ApiKind,
    /// Where the credential comes from.
    pub key_source: KeySource,
    /// The file holding the credential, where one applies.
    #[serde(default)]
    pub path: Option<String>,
    /// The major version of the service's HTTP API — the `/api/vN` path segment.
    ///
    /// Required for the `servarr` shape and read there, because that one shape
    /// spans two versions (Sonarr and Radarr at v3, Lidarr and Prowlarr at v1),
    /// so the version is data rather than a guess from a service's name. Absent
    /// for the other kinds, whose one fixed version their client already knows.
    #[serde(default)]
    pub version: Option<u32>,
}

/// The API shapes lemonfiber knows how to speak.
///
/// Four services share the `servarr` shape, which is what makes one client
/// enough for them. Bindery is its own kind deliberately: it is not a Servarr
/// application and Prowlarr's app sync does not reach it. Bazarr is its own for
/// the neighbouring reason: it is told about the \*arrs rather than being one of
/// them, in a form body a client of the shared shape could not send.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ApiKind {
    /// Sonarr, Radarr, Lidarr and Prowlarr.
    Servarr,
    /// `SABnzbd`.
    Sabnzbd,
    /// qBittorrent's `WebUI` API.
    Qbittorrent,
    /// Seerr.
    Seerr,
    /// Bindery.
    Bindery,
    /// Jellyfin — a media server whose account lemonfiber creates rather than a
    /// key it reads, so it has a `key_source` of `generated`.
    Jellyfin,
    /// Bazarr — the subtitle finder, which is told which \*arrs to watch.
    Bazarr,
    /// Audiobookshelf — a listening server whose first account lemonfiber creates,
    /// like Jellyfin's, so its `key_source` is `generated` too. The token it hands
    /// back on sign-in is stable, so it is read again rather than recorded twice.
    Audiobookshelf,
    /// `NZBHydra2` — the Usenet indexer aggregator, which starts with no authentication,
    /// so lemonfiber turns it on with an administrator it names and a password it mints.
    Nzbhydra2,
}

/// Where a service's credential comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum KeySource {
    /// The service writes it to an XML file lemonfiber reads.
    ConfigXml,
    /// The service writes it to an INI file lemonfiber reads.
    ConfigIni,
    /// The service writes it to a JSON file lemonfiber reads.
    ConfigJson,
    /// The service writes it to a YAML file lemonfiber reads.
    ConfigYaml,
    /// Retrieved over the service's own API once authenticated.
    ApiSettings,
    /// The service offers nothing durable, so lemonfiber mints and records one.
    Generated,
    /// The API needs no credential.
    None,
}

impl ApiKind {
    /// The name the manifest writes this adapter under, read from the one place it is
    /// spelled.
    #[must_use]
    pub fn name(self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_default()
    }

    /// Every adapter lemonfiber implements, which is the whole of what a plugin's
    /// service may name.
    pub const ALL: [Self; 9] = [
        Self::Servarr,
        Self::Sabnzbd,
        Self::Qbittorrent,
        Self::Seerr,
        Self::Bindery,
        Self::Jellyfin,
        Self::Bazarr,
        Self::Audiobookshelf,
        Self::Nzbhydra2,
    ];
}

impl KeySource {
    /// Every place a credential may come from.
    pub const ALL: [Self; 7] = [
        Self::ConfigXml,
        Self::ConfigIni,
        Self::ConfigJson,
        Self::ConfigYaml,
        Self::ApiSettings,
        Self::Generated,
        Self::None,
    ];
}
