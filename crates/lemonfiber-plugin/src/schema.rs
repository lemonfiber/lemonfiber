//! The shape of `plugin.toml`.
//!
//! These types mirror the contract field for field, and the published JSON Schema is
//! generated from them rather than written beside them — a schema that is written is
//! a claim about the reader, and a schema that is generated is a description of it.
//!
//! Every table refuses a field it does not know. Unknown names are tolerated when
//! reading an *answer* and refused when reading an *instruction*, and a manifest is
//! an instruction: a declaration skipped because nothing recognised it leaves a
//! plugin whose stated behaviour is narrower than its actual one, which is undetected
//! reach rather than a missing feature.

mod evidence;
mod recipe;

use serde::Deserialize;

use crate::conforming::nonconforming;
use crate::{is_compatible, Failure, SUPPORTED_SCHEMA_VERSIONS};

pub use evidence::{
    Claim, ClaimProbe, Contribution, Expect, Expected, ExpectedKind, Proof, Request,
};
pub use recipe::{Capture, Pair, Recipe, Step, StepCall, RUN};

/// A whole plugin manifest.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginManifest")]
pub struct Manifest {
    /// The manifest format generation.
    pub schema_version: u32,
    /// Who the plugin is, and where it came from.
    pub plugin: Plugin,
    /// What runs. One or more.
    ///
    /// More than one because a plugin is often a thing and the thing beside it: Plex
    /// and the reader of its watch history are one install and one uninstall to an
    /// operator, and are two containers on two tiers with two criticalities. A format
    /// permitting one forces the author of the pair to choose the wider tier for both
    /// halves, or to publish two plugins an operator has to keep in step by hand.
    #[serde(default, rename = "service")]
    pub services: Vec<Service>,
    /// The core capabilities this plugin claims, and the probes each is shown by.
    #[serde(default, rename = "claim")]
    pub claims: Vec<Claim>,
    /// How the stack's own proxy and dashboard reach its services. One each, at most.
    #[serde(default, rename = "wiring")]
    pub wirings: Vec<Wiring>,
    /// What must hold before it is installed.
    #[serde(default, rename = "proof")]
    pub proofs: Vec<Proof>,
    /// The rows it adds at the points lemonfiber publishes.
    #[serde(default, rename = "contribution")]
    pub contributions: Vec<Contribution>,
    /// The ordered calls that configure what it installed.
    #[serde(default, rename = "recipe")]
    pub recipes: Vec<Recipe>,
    /// Every value it will hold.
    #[serde(default, rename = "secret")]
    pub secrets: Vec<Secret>,
    /// Every bundled thing it will change.
    #[serde(default, rename = "override")]
    pub overrides: Vec<Override>,
    /// What the plugin needs of lemonfiber.
    #[serde(default)]
    pub requires: Option<Requires>,
}

impl Manifest {
    /// Read a plugin manifest from TOML, refusing a generation this build cannot read.
    ///
    /// The version gate runs before anything else looks at the contents, because
    /// fields mean different things across generations and a contents-first error
    /// would describe the wrong problem.
    ///
    /// # Errors
    ///
    /// Returns [`Failure::Syntax`] if the text is not a well-formed manifest,
    /// [`Failure::UnsupportedSchema`] if it declares a generation this build does not
    /// read, and [`Failure::Nonconforming`] if what it declares is not what the published
    /// schema describes.
    pub fn from_toml(text: &str) -> Result<Self, Failure> {
        // Read only the generation first. A newer generation may add or drop fields
        // the full parse would reject as unknown; reading it alone lets an old binary
        // say "you need a newer lemonfiber" rather than describe a field it has simply
        // never heard of.
        let generation: Generation = toml::from_str(text)?;
        if !is_compatible(generation.schema_version) {
            return Err(Failure::UnsupportedSchema {
                found: generation.schema_version,
                supported: SUPPORTED_SCHEMA_VERSIONS.to_vec(),
            });
        }

        // The published schema before the types. The read below stops at the first
        // fault it reaches and calls it a syntax error; holding the file to a schema
        // generated from these very types first is what lets an author be told
        // everything they have to change, each placed where they wrote it.
        let refused = nonconforming(text);
        if !refused.is_empty() {
            return Err(Failure::Nonconforming(refused));
        }

        Ok(toml::from_str(text)?)
    }
}

/// Just the manifest generation, read before the whole contract so the version gate
/// can speak before the field-by-field parse does.
///
/// No `deny_unknown_fields`: it must read a future manifest whose other fields this
/// build does not know, precisely so the generation can be checked first.
#[derive(Deserialize)]
struct Generation {
    schema_version: u32,
}

/// Who the plugin is, and where it came from.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Plugin {
    /// Unique, lowercase. The name it is installed and journalled under.
    pub id: String,
    /// Human-facing.
    pub name: String,
    /// Semver. The plugin's own content version, moved by its author.
    pub version: String,
    /// What it does *for the operator*.
    pub description: String,
    /// The consequence of its absence.
    pub without_it: String,
    /// Project URL, so the operator can judge the thing rather than the wrapper.
    pub upstream: String,
    /// SPDX identifier.
    ///
    /// Recorded and shown rather than constrained. The bundled set is a curated list
    /// this project stands behind and is OSI-licensed throughout; a plugin is the
    /// operator's own choice, and refusing to install proprietary software on somebody
    /// else's machine would be the tool standing between an operator and their stack.
    /// A plugin whose licence is absent is refused; one whose licence is merely not
    /// open is installed and said so.
    pub license: String,
    /// Which forms the service joins.
    pub forms: Vec<String>,
}

/// What runs.
///
/// Declared in the stack manifest's service vocabulary, restricted to these fields.
/// The restriction is the substance of it: the set of fields *is* the set of things a
/// plugin may ask for, so "what can this plugin reach" is answerable from the format
/// rather than from the instance.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginService")]
pub struct Service {
    /// Unique across the stack **and** every installed plugin.
    pub id: String,
    /// Human-facing.
    pub name: String,
    /// Registry path, without tag or digest.
    pub image: String,
    /// `sha256:…`. What actually runs.
    ///
    /// A tag is not a pin: it is a name the publisher can repoint, so the image
    /// reviewed and the image run can differ with nothing in the manifest changing.
    /// The digest is what lemonfiber writes into the generated entry.
    pub digest: String,
    /// The human-readable version the digest corresponds to. Recorded and shown;
    /// never resolved.
    pub tag: String,
    /// Primary UI or API port, absent for a service with no listener.
    #[serde(default)]
    pub port: Option<u16>,
    /// Which tier the port is published on.
    #[serde(default)]
    pub bind: Option<Bind>,
    /// How to tell the service has actually started.
    #[serde(default)]
    pub health: Option<Health>,
    /// How much its absence costs.
    pub criticality: Criticality,
    /// Which media types it handles, in the stack manifest's vocabulary.
    #[serde(default)]
    pub media_types: Vec<String>,
    /// Whether it needs the data root mounted.
    #[serde(default)]
    pub takes_data: bool,
    /// The capabilities this service claims.
    ///
    /// A core name comes from the published vocabulary and means the contracted thing
    /// that vocabulary defines; a plugin's own must be namespaced with the plugin's id
    /// and is inert until something asks for it.
    #[serde(default)]
    pub provides: Vec<String>,
    /// Where inside the container the one configuration directory is mounted.
    ///
    /// `/config` is a LinuxServer.io convention rather than a standard, and assuming
    /// it made a whole class of image uninstallable: a plugin whose image reads its
    /// configuration somewhere else would be generated a container mounting a
    /// directory the application never reads, passing its health probe and losing
    /// everything it had written the moment the container was replaced.
    ///
    /// The target is data; the source is still lemonfiber's, and there is still
    /// exactly one of it.
    #[serde(default)]
    pub config_path: Option<String>,
}

/// Where a service's own configuration directory lands when it names nowhere.
///
/// A LinuxServer.io convention rather than a standard, which is why it is the
/// fallback rather than the rule. Published as a constant because it is part of the
/// contract an author writes against: a manifest that leaves the field out has still
/// said where its directory goes, and the answer is this.
pub const CONFIGURATION: &str = "/config";

impl Service {
    /// Where inside the container this service's one configuration directory is
    /// mounted, whether or not the manifest said.
    ///
    /// Stated once, beside the field, so that the reader deciding what to write and
    /// the record saying what was written cannot default differently. Two callers
    /// each reaching for the constant is two places the convention can move.
    #[must_use]
    pub fn configuration(&self) -> &str {
        self.config_path.as_deref().unwrap_or(CONFIGURATION)
    }
}

/// How the stack's own services reach a plugin's service.
///
/// The tier governs, not the plugin. Only a `lan` service is proxied, because the
/// bundled policy is that an admin surface does not get a hostname — and a plugin
/// that could publish its own address could put an admin surface on the household
/// network without touching anything the web-security checks inspect.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Wiring {
    /// Which of this plugin's services it is about.
    ///
    /// Optional where the plugin declares one service, because there is nothing to
    /// choose between; required where it declares more, because a hostname is a fact
    /// about one service and a manifest that left it to be inferred would be inferring
    /// which of two the household reaches.
    #[serde(default)]
    pub service: Option<String>,
    /// The label in front of the operator's domain. A single DNS label — not a name,
    /// an address or a port. Defaults to the service's id.
    #[serde(default)]
    pub hostname: Option<String>,
    /// Which group on the bundled dashboard it appears under. Defaults to the group
    /// the stack uses for its tier.
    #[serde(default)]
    pub dashboard_group: Option<String>,
}

/// Where one service's own entry goes, once the manifest's defaults are taken.
///
/// A borrow of the manifest rather than a copy out of it, because nothing here is a
/// decision this crate makes: it is what the author wrote, with the two defaults the
/// contract states applied where they wrote nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry<'a> {
    /// The single DNS label the service answers on.
    pub hostname: &'a str,
    /// The group on the bundled dashboard it appears under, where one was named.
    pub group: Option<&'a str>,
}

impl Manifest {
    /// Where this service's own entry goes, from what the manifest declared.
    ///
    /// The hostname falls back to the service's id rather than the plugin's: a label
    /// is a fact about one container, and a default taken from the plugin would give
    /// two of its services one address. The group has no fallback this crate can
    /// state — which group a tier belongs to is the stack's answer, not the format's
    /// — so nothing declared is answered as nothing declared rather than guessed at.
    #[must_use]
    pub fn entry<'a>(&'a self, service: &'a Service) -> Entry<'a> {
        // The stanza about this service: one naming it, or one naming nothing,
        // which a plugin declaring a single service is allowed to write. A
        // plugin with two must name them, so an unnamed stanza there cannot be
        // about the wrong one — `naming::wired` refuses that before this runs.
        let declared = self.wirings.iter().find(|wiring| {
            wiring
                .service
                .as_deref()
                .is_none_or(|named| named == service.id)
        });
        Entry {
            hostname: declared
                .and_then(|wiring| wiring.hostname.as_deref())
                .unwrap_or(&service.id),
            group: declared.and_then(|wiring| wiring.dashboard_group.as_deref()),
        }
    }

    /// Which of this plugin's services a declaration asks, where that is settled.
    ///
    /// A proof, a contributed check and anything else written against one service
    /// may name it, and may leave it out where the plugin declares a single service
    /// because there is nothing to choose between. Nothing where it names one this
    /// manifest does not declare, or names none and the manifest declares several:
    /// both are refused when the manifest is read, and neither has a guess behind it
    /// worth making — the service is what a recording's digest is held to.
    ///
    /// Here rather than beside each reader, because three of them had written it and
    /// a fourth was about to. The one that fell behind would be answering a different
    /// question from the rest while looking like it agreed.
    #[must_use]
    pub fn asks(&self, named: Option<&str>) -> Option<&Service> {
        match named {
            None if self.services.len() == 1 => self.services.first(),
            None => None,
            Some(named) => self.services.iter().find(|service| service.id == named),
        }
    }
}

/// A value the plugin will hold.
///
/// Declared before anything can capture one, because the rule is that a secret
/// captured but not declared fails validation — and a rule that cannot be stated for
/// want of a field is not being enforced.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Secret {
    /// What the value is, within the plugin.
    pub id: String,
    /// Whose credential it is.
    pub of: String,
    /// What holding it is for.
    pub why: String,
}

/// A bundled thing the plugin will change.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Override {
    /// Which bundled setting is changed.
    pub id: String,
    /// What changing it is for.
    pub why: String,
}

/// What the plugin needs of lemonfiber.
///
/// Named capabilities and never a version. A plugin written a year ago against a
/// stack it has never met keeps working for exactly as long as the things it actually
/// uses still exist, and when it stops working the message says which thing went — a
/// version number cannot say that, because it conflates "older" with "missing
/// something you needed".
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Requires {
    /// What lemonfiber must offer for this plugin to be installable.
    #[serde(default)]
    pub capabilities: Vec<String>,
}

/// Which tier a plugin's port is published on.
///
/// A tier, not an address. lemonfiber renders it to an address exactly as it does for
/// a bundled service, so the two-tier policy stays a property of the system rather
/// than a request the plugin makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "PluginBind")]
pub enum Bind {
    /// Reachable only from the host.
    Loopback,
    /// Reachable from the local network.
    Lan,
}

/// How much a plugin's service costs when it is absent.
///
/// Four rather than the stack's five. `critical` means *its failure has consequences
/// outside the machine*, and in the bundled stack exactly one service holds it; it is
/// not a severity a contributor assigns to their own work, because the classification
/// drives how failures are reported and how hard lemonfiber tries to stop the operator
/// proceeding. A manifest declaring it is refused by name, with these four listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "PluginCriticality")]
pub enum Criticality {
    /// The form does not work without it.
    Core,
    /// The form works, badly.
    Important,
    /// Makes things better.
    Enhancing,
    /// Nice to have.
    Optional,
}

/// How to tell a plugin's service has started.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginHealth")]
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

/// The kinds of health probe a plugin's service can declare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "PluginHealthKind")]
pub enum HealthKind {
    /// Fetch a path on the service's port.
    Http,
    /// Open a connection to the service's port.
    Tcp,
    /// Trust the container's own reported state.
    Container,
}

#[cfg(test)]
pub(crate) mod tests;
