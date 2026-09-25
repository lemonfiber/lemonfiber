//! The shape of `stack.toml`.
//!
//! These types mirror the contract field for field. Nothing here interprets a
//! manifest — an unknown `api.kind` is a parse failure rather than a service
//! lemonfiber silently declines to wire up, because a typo that degrades into
//! "no API integration" is indistinguishable from meaning it.

use serde::{Deserialize, Serialize};

use crate::recognising::unrecognised;
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
    /// Every service this stack used to carry and no longer does.
    ///
    /// Optional, and deliberately: a stack that has never dropped anything has
    /// nothing to record, and requiring an empty table of it would make the record
    /// something to satisfy rather than something to read.
    #[serde(default)]
    pub removed: Vec<Removed>,
    /// Every link between two of this stack's services.
    ///
    /// Optional in the format because an operator's own stack directory stays
    /// readable without it, and because a stack written before this table existed
    /// is still a stack. It is not optional of the one this project ships: a link
    /// that is not declared here is one nothing can report on, substitute at, or
    /// show as the exception it is.
    #[serde(default, rename = "wiring")]
    pub wirings: Vec<Wiring>,
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
    /// Returns [`Error::Syntax`] if the text is not a well-formed manifest,
    /// [`Error::UnsupportedSchema`] if it declares a generation this build does
    /// not read, and [`Error::Unrecognised`] if it declares anything by a name
    /// this build does not know.
    pub fn from_toml(text: &str) -> Result<Self, Error> {
        // Read only the generation first. A newer generation may add or drop
        // fields the full parse would reject as unknown; reading it alone lets an
        // old binary say "you need a newer lemonfiber" rather than describe a
        // field it has simply never heard of.
        let generation: Generation = toml::from_str(text)?;
        if !is_compatible(generation.schema_version) {
            return Err(Error::UnsupportedSchema {
                found: generation.schema_version,
                supported: SUPPORTED_SCHEMA_VERSIONS.to_vec(),
            });
        }

        // Names before types. The read below stops at the first word it does not
        // know and calls it a syntax error; asking each declaration separately
        // first is what lets a fork be told everything it has to change, in the
        // vocabulary it wrote rather than the parser's.
        let unknown = unrecognised(text);
        if !unknown.is_empty() {
            return Err(Error::Unrecognised(unknown));
        }

        Ok(toml::from_str(text)?)
    }
}

/// Just the manifest generation, read before the whole contract so the version
/// gate can speak before the field-by-field parse does.
///
/// No `deny_unknown_fields`: it must read a future manifest whose other fields
/// this build does not know, precisely so the generation can be checked first.
#[derive(Deserialize)]
struct Generation {
    schema_version: u32,
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
///
/// Serialisable as well as readable, for the same reason [`Criticality`] is: it
/// reaches an operator. A profile left out of a closure is only half reported
/// without the provider it wanted.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Deserialize,
    Serialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
#[schemars(rename = "StackProtocol")]
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
    /// Where this service's own requests go, in the terms an operator would
    /// recognise — not in protocol names.
    ///
    /// An empty string is an answer: this service reaches nothing. That is not the
    /// same as the field being absent, which is the stack declining to say, and the
    /// two must never be read as one — an inventory of what leaves a machine that
    /// rendered "we were not told" as "nothing leaves" would be making a privacy
    /// claim out of its own ignorance.
    ///
    /// Optional because a stack written before this existed says nothing, and a build
    /// that refused such a manifest would refuse the stack it ships with. Where it is
    /// absent, lemonfiber answers from what it knows about the services it ships, and
    /// reports that it knows nothing where it does not.
    #[serde(default)]
    pub reaches: Option<String>,
    /// What it asks for out there, in the same terms.
    ///
    /// Declared with [`Self::reaches`] or not at all: one without the other is half
    /// an answer, and validation refuses it by name rather than quietly using the
    /// half that is there.
    #[serde(default)]
    pub asks_for: Option<String>,
    /// Which media types it handles.
    #[serde(default)]
    pub media_types: Vec<String>,
    /// What this service can be asked for, as named capabilities.
    ///
    /// An open list of strings here and checked for shape only, deliberately. The
    /// published vocabulary is the set these names come from, and it is generated from
    /// this field — a reader that held the set as well would be a cycle, and one that
    /// held a copy of it would be a second answer to what the vocabulary carries. The
    /// generator refuses a name the vocabulary does not carry, by name.
    #[serde(default)]
    pub provides: Vec<String>,
    /// Same-profile dependencies only.
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Extra kernel capabilities granted to the container, checked against an allow-list.
    ///
    /// Named `grants` rather than `capabilities` because the word is about to mean
    /// something else entirely: what a service *can do*, which is how a plugin declares
    /// itself and how wiring asks for a filler. Two meanings under one spelling is how a
    /// setting that decides what a container may do to the kernel gets read as a
    /// description of what it offers.
    ///
    /// The old spelling is still accepted, because an operator's own stack description is
    /// theirs and a rename is not a reason to refuse to read it.
    #[serde(default, alias = "capabilities")]
    pub grants: Vec<String>,
    /// True where the OS owns the lifecycle rather than Compose.
    #[serde(default)]
    pub host_managed: bool,
    /// The memory it expects to need, in MiB.
    ///
    /// The stack's estimate, and only ever shown as one: a form's footprint is the sum
    /// of these over the services it would start, and a figure read as a measurement
    /// is one an operator believes and acts on. Optional because a stack that says
    /// nothing has not said zero, and the services that are silent are named beside
    /// the sum rather than counted as costing nothing.
    #[serde(default)]
    pub memory_mib: Option<u32>,
}

/// A service this stack used to carry, and what became of it.
///
/// Kept beside the services rather than in a changelog because of who asks and when:
/// an operator who remembers a service and cannot find it is holding a gap, and a gap
/// is closed by a record that travels with the stack they are running rather than by a
/// file they would have to know exists. It versions with the stack for the same reason
/// every other per-service fact here does — the stack decided the removal, so the stack
/// is what carries the answer, and a lemonfiber release is not needed to say what
/// became of a service lemonfiber never chose.
///
/// `replaced_by` is optional because the honest answer is sometimes that nothing
/// replaced it. Recording a replacement that does not exist to avoid an empty field is
/// worse than the gap it fills.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Removed {
    /// The id it was declared under, which is the name an operator will look for.
    pub id: String,
    /// The `stack_version` whose catalogue no longer carries it.
    ///
    /// A stack version rather than a date, because the manifest already carries one
    /// and an operator asking what became of a service is holding a stack rather than
    /// a calendar: *which release of this stack stopped having it* is the question a
    /// date could only be translated back into.
    pub removed_in: String,
    /// Why it went — a sentence, not a word.
    pub reason: String,
    /// The service that took its place, where one did.
    #[serde(default)]
    pub replaced_by: Option<String>,
}

/// One link between two of the stack's services, and which of the two it names.
///
/// A link asks for a capability or it names a service, and the difference is the
/// whole subject. An ask is written against what the far end *does*, so anything
/// that stands in for it is reached by everything that asked and nothing else
/// changes; a name is written against what the far end *is*, which is sometimes the
/// honest answer and is always the exception.
///
/// Both halves of the pair are optional in the type and exactly one of them is
/// required by validation. Making it an enum in the parser would report a link that
/// carried both as a shape failure, and *this has two ends where it should have one*
/// is a sentence a parse error cannot say.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wiring {
    /// The service the link runs from — what asked.
    ///
    /// The name a report uses when nothing fills what was asked for. Without it an
    /// unfilled capability is a fact about the stack with nobody to tell, which is
    /// the failure at the point of use this exists instead of.
    pub by: String,
    /// The capability asked for, where this is an ask.
    #[serde(default)]
    pub asks: Option<String>,
    /// Whether the link reaches every service that fills it rather than the one.
    ///
    /// Some asks are for the one service that does a thing and some are for all of
    /// them, and how many claimants happen to exist today does not say which this
    /// is. A stack where one capability is declared four times over and another once
    /// would have both reported as the same answer, and only one of the two would be
    /// a link that worked.
    #[serde(default)]
    pub each: bool,
    /// Which claimant fills it, where the stack's own services contest it.
    ///
    /// A default the operator substitutes, not a rule: install order, precedence and
    /// recency are each a way of being right most of the time, and a choice written
    /// down with its reason beside it is what is done instead of them.
    #[serde(default)]
    pub filled_by: Option<String>,
    /// The service named, where this link is by name.
    #[serde(default)]
    pub to: Option<String>,
    /// Why it is by name, or why that claimant was chosen.
    ///
    /// Required of both, because an exception with no reason beside it reads as an
    /// oversight and a choice with no reason beside it reads as a rule.
    #[serde(default)]
    pub why: Option<String>,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
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
    /// Jellyfin — a media server whose account lemonfiber creates rather than a
    /// key it reads, so it has a `key_source` of `generated`.
    Jellyfin,
    /// Bazarr — the subtitle finder, which is told which \*arrs to watch.
    Bazarr,
    /// Audiobookshelf — a listening server whose first account lemonfiber creates,
    /// like Jellyfin's, so its `key_source` is `generated` too. The token it hands
    /// back on sign-in is stable, so it is read again rather than recorded twice.
    Audiobookshelf,
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
    /// The service writes it to a YAML file lemonfiber reads.
    ConfigYaml,
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
grants = ["NET_ADMIN"]
criticality = "core"
license = "GPL-2.0-only"
upstream = "https://github.com/qbittorrent/qBittorrent"
last_release = "2026-01-09"
describes = "Downloads torrents"
without_it = "No torrent downloads"
depends_on = ["gluetun"]
"#;

    /// One removal, appended to a manifest that declares the service it names as a
    /// replacement — so the record points at something an operator can actually run.
    const DROPPED: &str = r#"
[[removed]]
id = "readarr"
removed_in = "1.0.0"
reason = "Discontinued upstream in 2025; the project is archived and releases nothing."
replaced_by = "qbittorrent"
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

    /// A stack description written before the rename is still read.
    ///
    /// The field was called `capabilities` and the word is now needed for something
    /// else. An operator's own stack description is theirs, so the old spelling is
    /// accepted rather than refused — and because the manifest refuses unknown fields,
    /// dropping the alias would turn every stack written until now into a parse failure
    /// rather than a warning.
    #[test]
    fn a_stack_written_before_the_rename_still_reads_its_kernel_grants() {
        let older = MINIMAL.replace("grants = ", "capabilities = ");
        let granted = service(&older).map(|service| service.grants);
        let current = service(MINIMAL).map(|service| service.grants);

        assert_eq!(
            granted,
            Some(vec!["NET_ADMIN".to_owned()]),
            "the old spelling reads as what it always meant"
        );
        assert_eq!(
            granted, current,
            "and reads as the same thing the new one does"
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

    /// One profile per service is a shape rather than a rule to enforce: the field is a
    /// required string, so a service naming several is unparsable and one naming none is
    /// too. Worth a test of its own because it is the guarantee every closure rests on —
    /// a service in two profiles would start twice, or not at all, depending on which
    /// pass looked at it.
    #[test]
    fn a_service_declares_exactly_one_profile_and_no_other_shape_parses() {
        let several = format!("{MINIMAL}\n[[service]]\nprofiles = [\"torrent\", \"usenet\"]\n");
        assert!(
            Manifest::from_toml(&several).is_err(),
            "a service naming several profiles must not parse"
        );

        let none = MINIMAL.replace("profile = \"torrent\"", "");
        assert!(
            Manifest::from_toml(&none).is_err(),
            "a service naming no profile must not parse either"
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
    fn a_newer_generation_is_named_as_such_even_when_it_carries_unknown_fields() {
        // A future manifest declares a newer generation and, plausibly, fields
        // this build has never heard of. The version gate must speak first — "you
        // need a newer lemonfiber" — rather than the parser rejecting a field.
        let text = format!(
            "{}\n[[service]]\nfield_from_the_future = true\n",
            MINIMAL.replace("schema_version = 1", "schema_version = 99")
        );
        assert!(matches!(
            Manifest::from_toml(&text),
            Err(Error::UnsupportedSchema { found: 99, .. })
        ));
    }

    #[test]
    fn names_every_unrecognised_declaration_in_one_pass() {
        let text = MINIMAL
            .replace(r#"kind = "qbittorrent""#, r#"kind = "plex""#)
            .replace(r#"criticality = "core""#, r#"criticality = "vital""#);
        let refusal = Manifest::from_toml(&text)
            .err()
            .map(|refused| refused.to_string())
            .unwrap_or_default();
        assert!(refusal.contains("plex"), "names the first: {refusal}");
        assert!(refusal.contains("vital"), "names the second too: {refusal}");
    }

    /// A stack that has dropped something says what it was and what became of it.
    ///
    /// Read back whole rather than counted, because the count is the half that would
    /// still pass if every field arrived empty.
    #[test]
    fn reads_what_a_stack_has_dropped_and_what_took_its_place() {
        let text = format!("{MINIMAL}{DROPPED}");
        let recorded = parse(&text)
            .and_then(|manifest| manifest.removed.into_iter().next())
            .map(|removed| {
                (
                    removed.id,
                    removed.removed_in,
                    removed.reason.contains("Discontinued"),
                    removed.replaced_by,
                )
            });
        assert_eq!(
            recorded,
            Some((
                "readarr".to_owned(),
                "1.0.0".to_owned(),
                true,
                Some("qbittorrent".to_owned())
            ))
        );
    }

    /// Nothing replaced it is an answer, and the one a record has to be able to give
    /// — a stack forced to name a successor would name the nearest thing to hand.
    #[test]
    fn a_removal_with_nothing_in_its_place_records_that_rather_than_inventing_one() {
        let text = format!(
            "{MINIMAL}{}",
            DROPPED.replace("replaced_by = \"qbittorrent\"\n", "")
        );
        let replaced = parse(&text)
            .and_then(|manifest| manifest.removed.into_iter().next())
            .map(|removed| removed.replaced_by);
        assert_eq!(replaced, Some(None));
    }

    /// The table is optional, which is what keeps the two repositories from having to
    /// land together: a stack that has dropped nothing declares nothing and reads as
    /// having dropped nothing, rather than as a manifest missing a table.
    ///
    /// The generation does not move for it, and this is the assertion that pins that
    /// decision to something executable — a stack recording a removal reads under the
    /// generation every stack already declares, so neither repository is waiting on the
    /// other to renumber before it can land.
    #[test]
    fn a_stack_that_has_dropped_nothing_reads_as_having_dropped_nothing() {
        let recorded = parse(&format!("{MINIMAL}{DROPPED}")).map(|manifest| manifest.removed.len());
        let silent = parse(MINIMAL).map(|manifest| manifest.removed.len());

        assert_eq!(silent, Some(0));
        assert_eq!(
            recorded,
            Some(1),
            "and the same generation reads one that did"
        );
    }

    /// A field nobody declared on a removal is refused, the way it is everywhere else
    /// here: a misspelled `replaced_by` that is quietly dropped records a removal with
    /// no replacement, which is a different fact from the one somebody wrote.
    #[test]
    fn a_removal_declaring_a_field_this_build_does_not_know_is_refused() {
        let text = format!(
            "{MINIMAL}{}",
            DROPPED.replace("replaced_by = ", "replaced_with = ")
        );
        assert!(Manifest::from_toml(&text).is_err());
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
        assert_eq!(counted, Some((12, 11, 20)));
    }
}
