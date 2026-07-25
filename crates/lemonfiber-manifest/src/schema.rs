//! The shape of `stack.toml`.
//!
//! These types mirror the contract field for field. Nothing here interprets a
//! manifest — an unknown `api.kind` is a parse failure rather than a service
//! lemonfiber silently declines to wire up, because a typo that degrades into
//! "no API integration" is indistinguishable from meaning it.

use serde::{Deserialize, Serialize};

use crate::{is_compatible, Error, SUPPORTED_SCHEMA_VERSIONS};

/// A whole stack manifest.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// The manifest format generation.
    pub schema_version: u32,
    /// The content version — which services and forms this stack has.
    pub stack_version: String,
    /// The oldest `lemonfiber` this stack will work with.
    pub min_cli_version: String,
    /// Every declared profile.
    #[serde(default, rename = "profile")]
    pub profiles: Vec<Profile>,
    /// Every declared form.
    #[serde(default, rename = "form")]
    pub forms: Vec<Form>,
    /// Every declared service.
    #[serde(default, rename = "service")]
    pub services: Vec<Service>,
}

impl Manifest {
    /// Read a manifest from TOML, refusing a schema generation this build
    /// cannot read.
    ///
    /// The version gate runs before anything else looks at the contents,
    /// because fields mean different things across generations and a
    /// contents-first error would describe the wrong problem.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Syntax`] if the text is not a well-formed manifest, and
    /// [`Error::UnsupportedSchema`] if it declares a generation this build does
    /// not read.
    pub fn from_toml(text: &str) -> Result<Self, Error> {
        let manifest: Self = toml::from_str(text)?;
        if !is_compatible(manifest.schema_version) {
            return Err(Error::UnsupportedSchema {
                found: manifest.schema_version,
                supported: SUPPORTED_SCHEMA_VERSIONS.to_vec(),
            });
        }
        Ok(manifest)
    }
}

/// A Compose profile — one service's role in the stack.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    /// Unique, and matches a Compose profile name exactly.
    pub id: String,
    /// Human-facing name.
    pub name: String,
    /// Shown in form previews.
    pub description: String,
    /// The provider this profile cannot run without, where it needs one.
    ///
    /// Absent means it is never narrowed away. Declaring it here rather than
    /// letting the tool recognise profile names by sight is what keeps a
    /// renamed profile from silently losing its guard.
    #[serde(default)]
    pub protocol: Option<Protocol>,
}

/// A download provider a profile can depend on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    /// Needs a Usenet provider.
    Usenet,
    /// Needs a VPN and a torrent client.
    Torrent,
}

/// A named slice of the stack an operator can run.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Form {
    /// Unique.
    pub id: String,
    /// Human-facing name.
    pub name: String,
    /// One line, plain language.
    pub description: String,
    /// The complete closure, written out rather than inferred.
    pub profiles: Vec<String>,
    /// Whether this form may be combined with others.
    #[serde(default = "yes")]
    pub composable: bool,
}

/// One container in the stack.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Service {
    /// Unique, and matches the Compose service name.
    pub id: String,
    /// Human-facing name.
    pub name: String,
    /// Exactly one profile.
    pub profile: String,
    /// Image reference without a tag.
    pub image: String,
    /// Explicit version — a floating tag is a validation failure.
    pub tag: String,
    /// Primary UI or API port, absent for services with no listener.
    #[serde(default)]
    pub port: Option<u16>,
    /// Which interface the port is published on.
    #[serde(default)]
    pub bind: Option<Bind>,
    /// How to tell the service has actually started.
    #[serde(default)]
    pub health: Option<Health>,
    /// How lemonfiber talks to it when seeding.
    #[serde(default)]
    pub api: Option<Api>,
    /// How much its absence costs.
    pub criticality: Criticality,
    /// SPDX identifier.
    pub license: String,
    /// Project URL, for maintenance review.
    pub upstream: String,
    /// The latest release upstream has published, `YYYY-MM-DD`.
    pub last_release: String,
    /// What it does for the operator.
    pub describes: String,
    /// The consequence of not running it.
    pub without_it: String,
    /// Which media types it handles.
    #[serde(default)]
    pub media_types: Vec<String>,
    /// Same-profile dependencies only.
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Extra kernel capabilities, checked against an allow-list.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// True where the OS owns the lifecycle rather than Compose.
    #[serde(default)]
    pub host_managed: bool,
}

/// Which interface a service's port is published on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Bind {
    /// Reachable only from the host.
    Loopback,
    /// Reachable from the local network.
    Lan,
}

/// How much a service's absence costs.
///
/// Serialisable as well as readable, because it reaches an operator: a status
/// report that says a service is down without saying whether that matters
/// leaves them to guess, and the manifest already holds the answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Criticality {
    /// Its failure has consequences outside the machine.
    Critical,
    /// The form does not work without it.
    Core,
    /// The form works, badly.
    Important,
    /// Makes things better.
    Enhancing,
    /// Nice to have.
    Optional,
}

/// How to tell a service has started.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Health {
    /// Which kind of probe to use.
    pub kind: HealthKind,
    /// HTTP path, for `http` probes.
    #[serde(default)]
    pub path: Option<String>,
    /// How long to wait before calling it unhealthy.
    #[serde(default)]
    pub timeout_s: Option<u32>,
}

/// The kinds of health probe a service can declare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthKind {
    /// Fetch a path on the service's port.
    Http,
    /// Open a connection to the service's port.
    Tcp,
    /// Trust the container's own reported state.
    Container,
}

/// How lemonfiber talks to a service when seeding.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Api {
    /// Selects the client implementation.
    pub kind: ApiKind,
    /// Where the credential comes from.
    pub key_source: KeySource,
    /// The file holding the credential, where one applies.
    #[serde(default)]
    pub path: Option<String>,
}

/// The API shapes lemonfiber knows how to speak.
///
/// Four services share the `servarr` shape, which is what makes one client
/// enough for them. Bindery is its own kind deliberately: it is not a Servarr
/// application and Prowlarr's app sync does not reach it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
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
}

/// Where a service's credential comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KeySource {
    /// The service writes it to an XML file lemonfiber reads.
    ConfigXml,
    /// The service writes it to an INI file lemonfiber reads.
    ConfigIni,
    /// The service writes it to a JSON file lemonfiber reads.
    ConfigJson,
    /// Retrieved over the service's own API once authenticated.
    ApiSettings,
    /// The service offers nothing durable, so lemonfiber mints and records one.
    Generated,
    /// The API needs no credential.
    None,
}

/// `composable` defaults to true; serde needs a function to say so.
fn yes() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::{ApiKind, Bind, Criticality, HealthKind, KeySource, Manifest, Protocol, Service};
    use crate::Error;

    const MINIMAL: &str = r#"
schema_version = 1
stack_version = "1.0.0"
min_cli_version = "0.4.0"

[[profile]]
id = "torrent"
name = "Torrents"
description = "Torrent downloading, VPN-isolated"
protocol = "torrent"

[[form]]
id = "dl"
name = "Download"
description = "You have a link — fetch it."
profiles = ["torrent"]

[[service]]
id = "qbittorrent"
name = "qBittorrent"
profile = "torrent"
image = "lscr.io/linuxserver/qbittorrent"
tag = "5.0.3"
port = 8081
bind = "loopback"
health = { kind = "http", path = "/api/v2/app/version", timeout_s = 60 }
api = { kind = "qbittorrent", key_source = "generated" }
criticality = "core"
license = "GPL-2.0-only"
upstream = "https://github.com/qbittorrent/qBittorrent"
last_release = "2026-01-09"
describes = "Downloads torrents"
without_it = "No torrent downloads"
depends_on = ["gluetun"]
"#;

    /// Parsed, or absent.
    ///
    /// Assertions go through combinators rather than destructuring, because a
    /// `let … else` needs an arm for the case that cannot happen, and that arm
    /// is a line no test can ever reach.
    fn parse(text: &str) -> Option<Manifest> {
        Manifest::from_toml(text).ok()
    }

    /// The first declared service, for the tests that read one.
    fn service(text: &str) -> Option<Service> {
        parse(text).and_then(|manifest| manifest.services.into_iter().next())
    }

    #[test]
    fn reads_every_declared_collection() {
        let counted = parse(MINIMAL).map(|manifest| {
            (
                manifest.profiles.len(),
                manifest.forms.len(),
                manifest.services.len(),
                manifest.stack_version,
                manifest.min_cli_version,
            )
        });
        assert_eq!(
            counted,
            Some((1, 1, 1, "1.0.0".to_owned(), "0.4.0".to_owned()))
        );
    }

    #[test]
    fn reads_the_enumerated_fields_of_a_service() {
        let read = service(MINIMAL).map(|service| {
            (
                service.bind,
                service.criticality,
                service.port,
                service.depends_on,
            )
        });
        assert_eq!(
            read,
            Some((
                Some(Bind::Loopback),
                Criticality::Core,
                Some(8081),
                vec!["gluetun".to_owned()]
            ))
        );
    }

    #[test]
    fn reads_the_health_probe_a_service_declares() {
        let probe = service(MINIMAL)
            .and_then(|service| service.health)
            .map(|health| (health.kind, health.timeout_s));
        assert_eq!(probe, Some((HealthKind::Http, Some(60))));
    }

    #[test]
    fn reads_how_a_service_is_talked_to() {
        let api = service(MINIMAL)
            .and_then(|service| service.api)
            .map(|api| (api.kind, api.key_source, api.path));
        assert_eq!(
            api,
            Some((ApiKind::Qbittorrent, KeySource::Generated, None))
        );
    }

    #[test]
    fn a_profile_declares_the_provider_it_needs() {
        let declared = parse(MINIMAL)
            .and_then(|manifest| manifest.profiles.into_iter().next())
            .map(|profile| profile.protocol);
        assert_eq!(declared, Some(Some(Protocol::Torrent)));
    }

    #[test]
    fn a_profile_that_needs_no_provider_declares_none() {
        let text = MINIMAL.replace("protocol = \"torrent\"\n", "");
        let declared = Manifest::from_toml(&text)
            .ok()
            .and_then(|manifest| manifest.profiles.into_iter().next())
            .map(|profile| profile.protocol);
        assert_eq!(declared, Some(None));
    }

    #[test]
    fn a_form_is_composable_unless_it_says_otherwise() {
        let composable = parse(MINIMAL)
            .and_then(|manifest| manifest.forms.into_iter().next())
            .map(|form| form.composable);
        assert_eq!(composable, Some(true));
    }

    #[test]
    fn refuses_a_schema_generation_it_cannot_read() {
        let text = MINIMAL.replace("schema_version = 1", "schema_version = 99");
        let refusal = Manifest::from_toml(&text).err().map(|err| err.to_string());
        assert_eq!(
            refusal.as_deref(),
            Some("the manifest declares schema version 99, and this build reads [1]"),
            "the refusal names the version found and the versions supported"
        );
    }

    #[test]
    fn refuses_a_field_it_does_not_know() {
        let text = format!("{MINIMAL}\n[[service]]\nunknown_field = true\n");
        let refusal = Manifest::from_toml(&text)
            .err()
            .map(|err| matches!(err, Error::Syntax(_)));
        assert_eq!(refusal, Some(true));
    }

    #[test]
    fn parses_the_stack_this_binary_embeds() {
        let embedded = include_str!("../../../assets/media-stack/stack.toml");
        let counted = parse(embedded).map(|manifest| {
            (
                manifest.profiles.len(),
                manifest.forms.len(),
                manifest.services.len(),
            )
        });
        assert_eq!(counted, Some((12, 11, 19)));
    }
}
