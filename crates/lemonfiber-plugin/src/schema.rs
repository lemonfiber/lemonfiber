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

use lemonfiber_manifest::container::Container;
use serde::Deserialize;

use crate::conforming::nonconforming;
use crate::{is_compatible, Failure, SUPPORTED_SCHEMA_VERSIONS};

pub use evidence::{
    Claim, ClaimProbe, Contribution, Declaration, Declared, Expect, Expected, ExpectedKind, Proof,
    Request, Run, LONGEST_ARG, MOST_ARGS, RUN_TIMEOUT,
};
pub use lemonfiber_manifest::container::{Secret, Shape};
pub use recipe::{
    Capture, Condition, Input, On, Origin, Pair, Recipe, Retry, Step, StepCall, ALL_WAITING,
    CALL_DEADLINE, HEADER, LARGEST_ANSWER, LONGEST_WAIT, MOST_RETRIES, RUN,
};

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
    /// The capabilities its services ask for.
    #[serde(default, rename = "ask")]
    pub asking: Vec<Ask>,
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

lemonfiber_manifest::container_fields! {
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
    /// Where it is published on the host, absent for a service that publishes nothing.
    #[serde(default)]
    pub port: Option<u16>,
    /// Which tier every port it publishes is published on.
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
    /// The capabilities this service claims.
    ///
    /// A core name comes from the published vocabulary and means the contracted thing
    /// that vocabulary defines; a plugin's own must be namespaced with the plugin's id
    /// and is inert until something asks for it.
    #[serde(default)]
    pub provides: Vec<String>,
    /// Where it reaches outside the machine, in words an operator reads, as a stack
    /// service says it; declared with [`Self::asks_for`] or not at all.
    #[serde(default)]
    pub reaches: Option<String>,
    /// What it asks for out there, in the same terms; never blank.
    #[serde(default)]
    pub asks_for: Option<String>,
    /// The memory it expects to need, in MiB, never written as a limit.
    #[serde(default)]
    pub memory_mib: Option<u32>,
    /// The port it answers on inside the stack's network, where lemonfiber and the
    /// services that ask reach it. Required where `speaks` is.
    #[serde(default)]
    pub listens: Option<u16>,
    /// The port inside its container it speaks its contracts on, where that is not
    /// [`Self::listens`]: published on this machine's loopback alone, and reached by no
    /// published port, ask or proxy route.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract: Option<u16>,
    /// Each capability contract it answers, as `capability@major`: the service is an
    /// adapter, and lemonfiber asks it over those contracts.
    #[serde(default)]
    pub speaks: Vec<String>,
    /// The plugin's own service this adapter stands in front of: the upstream it answers
    /// its contracts for, whose pinned digest its conformance recordings are taken
    /// against. Where a service that speaks names none, it is its own upstream.
    #[serde(default)]
    pub fronts: Option<String>,
    /// Each field of the `upstream.json` lemonfiber writes for this adapter, to the secret
    /// it holds.
    #[serde(default)]
    pub upstream: std::collections::BTreeMap<String, lemonfiber_manifest::container::SecretOf>,
    /// The API this service answers the stack's other services in, by the name those
    /// services know it by: declared on the upstream an adapter of the same plugin fronts,
    /// where other services reach that upstream directly rather than through the adapter.
    #[serde(default)]
    pub native: Option<String>,
    /// The id of the stack's own service this service moves in place of, which is its own
    /// id: it keeps that service's container, configuration and data, and may take its
    /// port and the name the proxy answers for it.
    #[serde(default)]
    pub replaces: Option<String>,
}
}

/// Where a service's own configuration directory lands when it names nowhere.
///
/// A LinuxServer.io convention rather than a standard, which is why it is the
/// fallback rather than the rule. Published as a constant because it is part of the
/// contract an author writes against: a manifest that leaves the field out has still
/// said where its directory goes, and the answer is this.
pub const CONFIGURATION: &str = lemonfiber_manifest::container::CONFIGURATION;

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

    /// Its container fields, as the one model every service's are, its settings in
    /// `plugin`'s namespace and `secrets` the ones its manifest declares.
    #[must_use]
    pub fn container<'a>(&'a self, plugin: &'a str, secrets: &'a [Secret]) -> Container<'a> {
        Container {
            id: &self.id,
            namespace: plugin,
            port: self.port,
            listens: self.listens,
            publish: &self.publish,
            takes_data: self.takes_data,
            config_path: self.config_path.as_deref(),
            config_ro: self.config_ro,
            mounts: &self.mount,
            shares_config: self.shares_config.as_deref(),
            rebuilt: &self.rebuilt,
            fronts: self.fronts.as_deref(),
            tmpfs: &self.tmpfs,
            env: &self.env,
            secrets,
            user_mode: self.user_mode,
            posture: self.posture,
            mem_limit_mib: self.mem_limit_mib,
            network: self.network.as_ref(),
            shape: self.shape,
            certificate: self.certificate.as_ref(),
            provides: &self.provides,
            speaks: !self.speaks.is_empty(),
        }
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
    /// The line beside its entry on the bundled dashboard. Defaults to the plugin's
    /// description.
    #[serde(default)]
    pub dashboard_description: Option<String>,
    /// The icon its entry on the bundled dashboard shows, named from the dashboard's own
    /// icon set. Defaults to the service's id with `.png` after it.
    #[serde(default)]
    pub dashboard_icon: Option<String>,
    /// Whether it has no entry on the bundled dashboard.
    #[serde(default)]
    pub dashboard_hidden: bool,
}

/// One capability a plugin's service asks for.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginAsk")]
pub struct Ask {
    /// The plugin's own service that asks; optional where the plugin declares one.
    #[serde(default)]
    pub service: Option<String>,
    /// The core capability asked for.
    pub capability: String,
    /// Whether every service that fills it is reached rather than one.
    #[serde(default)]
    pub each: bool,
    /// The upstream's own API the service talks in, which the ask then reaches in place
    /// of the adapter fronting it.
    #[serde(default)]
    pub native: Option<String>,
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
    /// The line beside its entry on the bundled dashboard: the one named, or the
    /// plugin's description.
    pub description: &'a str,
    /// The icon its entry on the bundled dashboard shows, where one was named.
    pub icon: Option<&'a str>,
    /// Whether it has no entry on the bundled dashboard.
    pub hidden: bool,
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
            description: declared
                .and_then(|wiring| wiring.dashboard_description.as_deref())
                .unwrap_or(&self.plugin.description),
            icon: declared.and_then(|wiring| wiring.dashboard_icon.as_deref()),
            hidden: declared.is_some_and(|wiring| wiring.dashboard_hidden),
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
/// `critical` means *its failure has consequences outside the machine*, which is the
/// egress guard's alone: a service declares it only with `shape = "egress-guard"`, and
/// one that declares it without is refused naming the shape and the four it may declare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "PluginCriticality")]
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

impl Criticality {
    /// The criticalities a service may declare without the egress guard's shape.
    pub const UNSHAPED: [Self; 4] = [Self::Core, Self::Important, Self::Enhancing, Self::Optional];

    /// The name a manifest gives it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::Core => "core",
            Self::Important => "important",
            Self::Enhancing => "enhancing",
            Self::Optional => "optional",
        }
    }
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

mod speaking;

#[cfg(test)]
pub(crate) mod tests;
