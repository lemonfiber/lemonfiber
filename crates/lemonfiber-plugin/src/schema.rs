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
use crate::{is_compatible, Error, SUPPORTED_SCHEMA_VERSIONS};

pub use evidence::{Claim, ClaimProbe, Contribution, Expect, Expected, Kind, Proof, Request};
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
    /// What runs. Exactly one, in this generation.
    #[serde(default, rename = "service")]
    pub services: Vec<Service>,
    /// The core capabilities this plugin claims, and the probes each is shown by.
    #[serde(default, rename = "claim")]
    pub claims: Vec<Claim>,
    /// How the stack's own proxy and dashboard reach it.
    #[serde(default)]
    pub wiring: Option<Wiring>,
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
    /// Returns [`Error::Syntax`] if the text is not a well-formed manifest,
    /// [`Error::UnsupportedSchema`] if it declares a generation this build does not
    /// read, and [`Error::Nonconforming`] if what it declares is not what the published
    /// schema describes.
    pub fn from_toml(text: &str) -> Result<Self, Error> {
        // Read only the generation first. A newer generation may add or drop fields
        // the full parse would reject as unknown; reading it alone lets an old binary
        // say "you need a newer lemonfiber" rather than describe a field it has simply
        // never heard of.
        let generation: Generation = toml::from_str(text)?;
        if !is_compatible(generation.schema_version) {
            return Err(Error::UnsupportedSchema {
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
            return Err(Error::Nonconforming(refused));
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

/// How the stack's own services reach a plugin's service.
///
/// The tier governs, not the plugin. Only a `lan` service is proxied, because the
/// bundled policy is that an admin surface does not get a hostname — and a plugin
/// that could publish its own address could put an admin surface on the household
/// network without touching anything the web-security checks inspect.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Wiring {
    /// The label in front of the operator's domain. A single DNS label — not a name,
    /// an address or a port. Defaults to the plugin's id.
    #[serde(default)]
    pub hostname: Option<String>,
    /// Which group on the bundled dashboard it appears under. Defaults to the group
    /// the stack uses for its tier.
    #[serde(default)]
    pub dashboard_group: Option<String>,
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
#[serde(rename_all = "lowercase")]
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
#[serde(rename_all = "lowercase")]
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
#[serde(rename_all = "lowercase")]
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
pub(crate) mod tests {
    use super::{Bind, Contribution, Criticality, Expected, HealthKind, Kind, Manifest};
    use crate::Error;

    /// A manifest declaring every block the contract carries.
    ///
    /// Whole rather than minimal, because what these tests are for is that the types
    /// mirror the contract field for field — and a fixture carrying only the required
    /// half would leave the optional half describing nothing.
    pub(crate) const WHOLE: &str = r#"
schema_version = 1

[plugin]
id          = "komga"
name        = "Komga"
version     = "1.2.0"
description = "Reads your comics on any browser"
without_it  = "Files on disk, no way to read them"
upstream    = "https://github.com/gotson/komga"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "komga"
name        = "Komga"
image       = "docker.io/gotson/komga"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.11.0"
port        = 25600
bind        = "lan"
health      = { kind = "http", path = "/actuator/health", timeout_s = 90 }
criticality = "important"
media_types = ["comics"]
takes_data  = true
provides    = ["media.serve", "komga:kobo-sync"]
config_path = "/config"

[[claim]]
capability = "media.serve"

[[claim.probe]]
id      = "guarded"
request = { method = "GET", path = "/api/v1/series" }
expect  = { status = 401 }
fixture = "fixtures/media-serve-guarded.json"

[[claim.probe]]
id      = "catalogue"
request = { method = "GET", path = "/api/v1/series" }
expect  = { status = 200, json_has_keys = ["content"], json_types = { content = "list" }, json_at_least = { totalElements = 1 }, json_array_min = 1, json_is_absent = false, content_type = "application/json", body_starts_with = "{" }
fixture = "fixtures/media-serve-catalogue.json"

[wiring]
hostname        = "comics"
dashboard_group = "Library"

[[proof]]
id      = "komga.serves"
title   = "Komga answers on its declared health path"
request = { method = "GET", path = "/actuator/health" }
expect  = { status = 200, json = { status = "UP", claimed = true, libraries = 3 } }
fixture = "fixtures/health.json"
why     = "The path the health probe asks for is one this image serves."

[[contribution]]
at        = "doctor.check"
id        = "komga:claimed"
title     = "Komga has an administrator, so nobody else can become one"
category  = "services"
request   = { method = "GET", path = "/api/v1/claim" }
expect    = { status = 200, json = { isClaimed = true } }
fixture   = "fixtures/api-v1-claim-claimed.json"
timeout_s = 10
service   = "komga"
why       = "An unclaimed Komga hands administrator to whoever asks first."

[[contribution]]
at     = "doctor.remedy"
id     = "komga:claim-it"
for    = "komga:claimed"
action = "Open Komga and create the administrator account"
detail = "POST /api/v1/claim with an email and a password creates it."
why    = "Until somebody does, the first caller on the household network becomes it."

[[recipe]]
id    = "adopt-existing-library"
title = "Point it at the comics the stack already files"
why   = "The stack already files comics, and a fresh Komga knows nothing about them."

[[recipe.step]]
id      = "sign-in"
call    = { method = "POST", to = "komga", path = "/api/v1/login", body = "{\"password\": \"read from the operator's store\"}" }
expect  = { status = 200 }
capture = [{ name = "token", from = "json.token", origin = "stack-service" }]

[[recipe.step]]
id      = "create"
call    = { method = "POST", to = "komga", path = "/api/v1/libraries", headers = { Authorization = "Bearer {{token}}" }, body = "{\"name\": \"Comics\"}" }
expect  = { status = 200 }
capture = [{ name = "library", from = "json.id", origin = "stack-service" }]

[[recipe.pair]]
value = "token"
to    = "komga"

[[secret]]
id  = "api-key"
of  = "komga"
why = "Read the library counts the dashboard panel shows"

[[override]]
id  = "homepage.services"
why = "Add its own entry to the bundled dashboard"

[requires]
capabilities = ["doctor.contribute", "recipe.run"]
"#;

    /// Parsed, or absent.
    ///
    /// Assertions go through combinators rather than destructuring, because a
    /// `let … else` needs an arm for the case that cannot happen, and that arm is a
    /// line no test can ever reach.
    fn parse(text: &str) -> Option<Manifest> {
        Manifest::from_toml(text).ok()
    }

    #[test]
    fn reads_every_block_the_contract_declares() {
        let counted = parse(WHOLE).map(|manifest| {
            (
                manifest.services.len(),
                manifest.claims.len(),
                manifest.proofs.len(),
                manifest.contributions.len(),
                manifest.recipes.len(),
                manifest.secrets.len(),
                manifest.overrides.len(),
            )
        });
        assert_eq!(counted, Some((1, 1, 1, 2, 1, 1, 1)));
    }

    #[test]
    fn reads_who_the_plugin_is_and_where_it_came_from() {
        let read = parse(WHOLE).map(|manifest| {
            (
                manifest.plugin.id,
                manifest.plugin.name,
                manifest.plugin.version,
                manifest.plugin.description,
                manifest.plugin.without_it,
                manifest.plugin.upstream,
                manifest.plugin.license,
                manifest.plugin.forms,
            )
        });
        assert_eq!(
            read,
            Some((
                "komga".to_owned(),
                "Komga".to_owned(),
                "1.2.0".to_owned(),
                "Reads your comics on any browser".to_owned(),
                "Files on disk, no way to read them".to_owned(),
                "https://github.com/gotson/komga".to_owned(),
                "MIT".to_owned(),
                vec!["library".to_owned()],
            ))
        );
    }

    #[test]
    fn reads_what_the_plugin_runs() {
        let read = parse(WHOLE)
            .and_then(|manifest| manifest.services.into_iter().next())
            .map(|service| {
                (
                    service.id,
                    service.image,
                    service.digest.starts_with("sha256:"),
                    service.tag,
                    service.port,
                    service.bind,
                    service.criticality,
                    service.media_types,
                    service.takes_data,
                    service.provides,
                    service.config_path,
                )
            });
        assert_eq!(
            read,
            Some((
                "komga".to_owned(),
                "docker.io/gotson/komga".to_owned(),
                true,
                "1.11.0".to_owned(),
                Some(25600),
                Some(Bind::Lan),
                Criticality::Important,
                vec!["comics".to_owned()],
                true,
                vec!["media.serve".to_owned(), "komga:kobo-sync".to_owned()],
                Some("/config".to_owned()),
            ))
        );
    }

    #[test]
    fn reads_the_health_probe_a_plugin_declares() {
        let probe = parse(WHOLE)
            .and_then(|manifest| manifest.services.into_iter().next())
            .and_then(|service| service.health)
            .map(|health| (health.kind, health.path, health.timeout_s));
        assert_eq!(
            probe,
            Some((
                HealthKind::Http,
                Some("/actuator/health".to_owned()),
                Some(90)
            ))
        );
    }

    /// A claim names a capability and binds each probe to a request of its own.
    ///
    /// Read back whole rather than counted, because the count is the half that would
    /// still pass if every field arrived empty.
    #[test]
    fn a_claim_binds_each_probe_to_a_request_and_a_recording() {
        let bound = parse(WHOLE)
            .and_then(|manifest| manifest.claims.into_iter().next())
            .map(|claim| {
                let bindings: Vec<(String, String, String, Option<u16>, String)> = claim
                    .probes
                    .into_iter()
                    .map(|probe| {
                        (
                            probe.id,
                            probe.request.method,
                            probe.request.path,
                            probe.expect.status,
                            probe.fixture,
                        )
                    })
                    .collect();
                (claim.capability, bindings)
            });
        assert_eq!(
            bound,
            Some((
                "media.serve".to_owned(),
                vec![
                    (
                        "guarded".to_owned(),
                        "GET".to_owned(),
                        "/api/v1/series".to_owned(),
                        Some(401),
                        "fixtures/media-serve-guarded.json".to_owned(),
                    ),
                    (
                        "catalogue".to_owned(),
                        "GET".to_owned(),
                        "/api/v1/series".to_owned(),
                        Some(200),
                        "fixtures/media-serve-catalogue.json".to_owned(),
                    ),
                ]
            ))
        );
    }

    /// Every kind of body constraint the shared vocabulary has, read off one binding.
    ///
    /// Together rather than one test each, because what is being checked is that the
    /// vocabulary is *whole* — a kind the type is missing reads exactly like a kind
    /// the manifest did not declare.
    #[test]
    fn every_kind_of_body_constraint_is_read() {
        let read = parse(WHOLE)
            .and_then(|manifest| manifest.claims.into_iter().next())
            .and_then(|claim| claim.probes.into_iter().nth(1))
            .map(|probe| probe.expect);
        let expected = read.map(|expect| {
            (
                expect.json_has_keys,
                expect
                    .json_types
                    .and_then(|types| types.get("content").copied()),
                expect
                    .json_at_least
                    .and_then(|least| least.get("totalElements").copied()),
                expect.json_array_min,
                expect.json_is_absent,
                expect.content_type,
                expect.body_starts_with,
            )
        });
        assert_eq!(
            expected,
            Some((
                Some(vec!["content".to_owned()]),
                Some(Kind::List),
                Some(1),
                Some(1),
                Some(false),
                Some("application/json".to_owned()),
                Some("{".to_owned()),
            ))
        );
    }

    /// The three things an exact value can be, read off one proof.
    #[test]
    fn an_exact_value_is_a_word_a_flag_or_a_number() {
        let held = parse(WHOLE)
            .and_then(|manifest| manifest.proofs.into_iter().next())
            .and_then(|proof| proof.expect.json)
            .map(|json| {
                (
                    json.get("status").cloned(),
                    json.get("claimed").cloned(),
                    json.get("libraries").cloned(),
                )
            });
        assert_eq!(
            held,
            Some((
                Some(Expected::Word("UP".to_owned())),
                Some(Expected::Flag(true)),
                Some(Expected::Number(3)),
            ))
        );
    }

    #[test]
    fn reads_what_a_proof_asks_and_why_it_is_worth_asking() {
        let read = parse(WHOLE)
            .and_then(|manifest| manifest.proofs.into_iter().next())
            .map(|proof| {
                (
                    proof.id,
                    proof.title,
                    proof.request.method,
                    proof.fixture,
                    proof.why.contains("health probe"),
                )
            });
        assert_eq!(
            read,
            Some((
                "komga.serves".to_owned(),
                "Komga answers on its declared health path".to_owned(),
                "GET".to_owned(),
                Some("fixtures/health.json".to_owned()),
                true,
            ))
        );
    }

    /// Which fields one row of a contribution filled in.
    ///
    /// Said as a list of names rather than as a tuple of options, because what the two
    /// rows differ in is *which* of the union they use — and a reader comparing two
    /// columns of `None` against two columns of `Some` is doing that translation by eye.
    fn filled(row: &Contribution) -> Vec<&'static str> {
        [
            ("title", row.title.is_some()),
            ("category", row.category.is_some()),
            ("request", row.request.is_some()),
            ("expect", row.expect.is_some()),
            ("why", row.why.is_some()),
            ("fixture", row.fixture.is_some()),
            ("timeout_s", row.timeout_s.is_some()),
            ("service", row.service.is_some()),
            ("for", row.about.is_some()),
            ("action", row.action.is_some()),
            ("detail", row.detail.is_some()),
        ]
        .into_iter()
        .filter_map(|(field, given)| given.then_some(field))
        .collect()
    }

    /// A contributed check and the remedy that answers it are one block, twice.
    ///
    /// The row belongs to the point rather than to this type, so the two shapes share
    /// a table and are told apart by `at` — which is what keeps the required set
    /// published in one place instead of restated in two.
    #[test]
    fn a_contribution_carries_the_row_its_point_declares() {
        let rows: Vec<Contribution> = parse(WHOLE)
            .map(|manifest| manifest.contributions)
            .unwrap_or_default();
        let named: Vec<(&str, &str)> = rows
            .iter()
            .map(|row| (row.at.as_str(), row.id.as_str()))
            .collect();
        assert_eq!(
            named,
            vec![
                ("doctor.check", "komga:claimed"),
                ("doctor.remedy", "komga:claim-it"),
            ]
        );

        let check = rows.first().map(filled);
        assert_eq!(
            check,
            Some(vec![
                "title",
                "category",
                "request",
                "expect",
                "why",
                "fixture",
                "timeout_s",
                "service",
            ])
        );

        let remedy = rows.get(1).map(filled);
        assert_eq!(remedy, Some(vec!["why", "for", "action", "detail"]));
        assert_eq!(
            rows.get(1).and_then(|row| row.about.clone()),
            Some("komga:claimed".to_owned())
        );
    }

    #[test]
    fn a_recipe_declares_its_calls_its_captures_and_where_each_value_may_go() {
        let read = parse(WHOLE)
            .and_then(|manifest| manifest.recipes.into_iter().next())
            .map(|recipe| {
                let step = recipe.steps.into_iter().find(|step| step.id == "create");
                let pair = recipe.pairs.into_iter().next();
                (
                    recipe.id,
                    recipe.title,
                    recipe.why.is_empty(),
                    step.map(|step| {
                        let capture = step.capture.into_iter().next();
                        (
                            step.id,
                            step.call.method,
                            step.call.to,
                            step.call.path,
                            step.call
                                .headers
                                .and_then(|carried| carried.get("Authorization").cloned()),
                            step.call.body.is_some(),
                            step.expect.and_then(|expect| expect.status),
                            capture.map(|one| (one.name, one.from, one.origin)),
                        )
                    }),
                    pair.map(|pair| (pair.value, pair.to)),
                )
            });
        assert_eq!(
            read,
            Some((
                "adopt-existing-library".to_owned(),
                "Point it at the comics the stack already files".to_owned(),
                false,
                Some((
                    "create".to_owned(),
                    "POST".to_owned(),
                    "komga".to_owned(),
                    "/api/v1/libraries".to_owned(),
                    Some("Bearer {{token}}".to_owned()),
                    true,
                    Some(200),
                    Some((
                        "library".to_owned(),
                        "json.id".to_owned(),
                        "stack-service".to_owned()
                    )),
                )),
                Some(("token".to_owned(), "komga".to_owned())),
            ))
        );
    }

    #[test]
    fn reads_what_it_will_hold_what_it_will_change_and_what_it_needs() {
        let read = parse(WHOLE).map(|manifest| {
            (
                manifest
                    .secrets
                    .into_iter()
                    .next()
                    .map(|secret| (secret.id, secret.of, secret.why.is_empty())),
                manifest
                    .overrides
                    .into_iter()
                    .next()
                    .map(|changed| (changed.id, changed.why.is_empty())),
                manifest.requires.map(|requires| requires.capabilities),
            )
        });
        assert_eq!(
            read,
            Some((
                Some(("api-key".to_owned(), "komga".to_owned(), false)),
                Some(("homepage.services".to_owned(), false)),
                Some(vec![
                    "doctor.contribute".to_owned(),
                    "recipe.run".to_owned()
                ]),
            ))
        );
    }

    #[test]
    fn reads_how_the_stack_is_told_to_reach_it() {
        let read = parse(WHOLE)
            .and_then(|manifest| manifest.wiring)
            .map(|wiring| (wiring.hostname, wiring.dashboard_group));
        assert_eq!(
            read,
            Some((Some("comics".to_owned()), Some("Library".to_owned())))
        );
    }

    /// The optional half is optional, and reads as absent rather than as a fault.
    #[test]
    fn a_plugin_that_declares_only_what_it_must_still_reads() {
        let bare = r#"
schema_version = 1

[plugin]
id          = "tiny"
name        = "Tiny"
version     = "0.1.0"
description = "Does one thing"
without_it  = "That thing is not done"
upstream    = "https://example.invalid/tiny"
license     = "MIT"
forms       = ["library"]
"#;
        let read = parse(bare).map(|manifest| {
            (
                manifest.services.len(),
                manifest.wiring.is_some(),
                manifest.requires.is_some(),
                manifest.recipes.len(),
            )
        });
        assert_eq!(read, Some((0, false, false, 0)));
    }

    /// A service that answers its application shell for every unimplemented path.
    ///
    /// The shape two of the published plugins already carry, and the reason
    /// `json_is_absent` is a flag rather than a list of keys: what has to be said is
    /// that the body was not a document at all. A status alone proves nothing against
    /// such a service, because the shell comes back `200` whether the API behind it
    /// exists or not.
    #[test]
    fn a_proof_can_say_the_answer_was_not_json_at_all() {
        let text = WHOLE.replace(
            r#"expect  = { status = 200, json = { status = "UP", claimed = true, libraries = 3 } }"#,
            r#"expect  = { status = 200, content_type = "text/html", json_is_absent = true, body_starts_with = "<!DOCTYPE html>" }"#,
        );
        let read = parse(&text)
            .and_then(|manifest| manifest.proofs.into_iter().next())
            .map(|proof| {
                (
                    proof.expect.json_is_absent,
                    proof.expect.content_type,
                    proof.expect.body_starts_with,
                    proof.expect.json.is_none(),
                )
            });
        assert_eq!(
            read,
            Some((
                Some(true),
                Some("text/html".to_owned()),
                Some("<!DOCTYPE html>".to_owned()),
                true,
            ))
        );
    }

    #[test]
    fn refuses_a_schema_generation_it_cannot_read() {
        let text = WHOLE.replace("schema_version = 1", "schema_version = 99");
        let refusal = Manifest::from_toml(&text).err().map(|err| err.to_string());
        assert_eq!(
            refusal.as_deref(),
            Some("the plugin manifest declares schema version 99, and this build reads [1]"),
            "the refusal names the version found and the versions supported"
        );
    }

    #[test]
    fn a_newer_generation_is_named_as_such_even_when_it_carries_unknown_fields() {
        let text = format!(
            "{}\nfield_from_the_future = true\n",
            WHOLE.replace("schema_version = 1", "schema_version = 99")
        );
        assert!(matches!(
            Manifest::from_toml(&text),
            Err(Error::UnsupportedSchema { found: 99, .. })
        ));
    }

    /// A field nobody declared is refused rather than skipped.
    ///
    /// The declaration is the entire basis for saying what a plugin may do, so a
    /// tolerated unknown is a plugin whose stated behaviour is narrower than its
    /// actual one.
    #[test]
    fn refuses_a_field_it_does_not_know() {
        let text = WHOLE.replace("takes_data  = true", "takes_data = true\nprivileged = true");
        let refusal = Manifest::from_toml(&text)
            .err()
            .map(|refused| refused.to_string())
            .unwrap_or_default();
        assert!(
            refusal.contains("service komga.privileged"),
            "names the field and the entry it was declared on: {refusal}"
        );
        assert!(
            refusal.contains("config_path"),
            "and lists what may be declared instead: {refusal}"
        );
    }

    /// The names it does not know are reported together, not one per run.
    #[test]
    fn names_every_unrecognised_declaration_in_one_pass() {
        let text = WHOLE
            .replace(r#"bind        = "lan""#, r#"bind        = "wan""#)
            .replace(
                r#"criticality = "important""#,
                r#"criticality = "critical""#,
            );
        let refusal = Manifest::from_toml(&text)
            .err()
            .map(|refused| refused.to_string())
            .unwrap_or_default();
        assert!(refusal.contains("`wan`"), "names the first: {refusal}");
        assert!(
            refusal.contains("`critical`"),
            "names the second too: {refusal}"
        );
    }

    #[test]
    fn a_file_that_is_not_a_manifest_at_all_is_a_syntax_error() {
        assert!(matches!(
            Manifest::from_toml("= not toml"),
            Err(Error::Syntax(_))
        ));
    }
}
