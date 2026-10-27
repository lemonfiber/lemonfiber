//! The APIs lemonfiber speaks, and where a credential for one may come from.

use serde::{Deserialize, Serialize};

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
    /// key it reads.
    Jellyfin,
    /// Bazarr — the subtitle finder, which is told which \*arrs to watch.
    Bazarr,
    /// Audiobookshelf — a listening server whose first account lemonfiber creates,
    /// like Jellyfin's. The token it hands back on sign-in is stable, so it is read
    /// again rather than recorded twice.
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
    /// The name the manifest writes this adapter under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Servarr => "servarr",
            Self::Sabnzbd => "sabnzbd",
            Self::Qbittorrent => "qbittorrent",
            Self::Seerr => "seerr",
            Self::Bindery => "bindery",
            Self::Jellyfin => "jellyfin",
            Self::Bazarr => "bazarr",
            Self::Audiobookshelf => "audiobookshelf",
            Self::Nzbhydra2 => "nzbhydra2",
        }
    }

    /// The adapter the manifest writes as `name`, where lemonfiber implements one.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
    }

    /// Every adapter lemonfiber implements, which is the whole of what a recipe step may
    /// be reached through.
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
