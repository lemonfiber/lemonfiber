//! What a plugin asks, and what the answer has to be.
//!
//! One vocabulary under three names. A claim's probe, a proof and a contributed
//! check each ask a question of a service and say what the answer must carry, so they
//! share a [`Request`] and an [`Expect`] rather than each having a shape of its own —
//! which is what keeps "a status alone is not evidence" one rule instead of three.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The core capability a plugin claims, and the probes it is demonstrated by.
///
/// The vocabulary owns what must be shown; this owns where to ask. A capability is
/// one contract with many claimants, each answering at a path of its own, so the
/// published probe declares the question and the statuses that answer it, and the
/// binding declares the method, the path and the recorded response.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    /// A core name, which must also appear in the service's `provides`.
    pub capability: String,
    /// One binding per probe the capability declares. Every one of them, exactly once.
    #[serde(default, rename = "probe")]
    pub probes: Vec<ClaimProbe>,
}

/// One probe of a claimed capability, bound to a request on this plugin's service.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClaimProbe {
    /// Names a probe the capability declares.
    pub id: String,
    /// Method and path on this plugin's own service.
    pub request: Request,
    /// What the answer must be, within what the probe permits.
    pub expect: Expect,
    /// The recorded response.
    ///
    /// Required here where it is optional on a proof: a claim nobody can demonstrate
    /// without owning the service is a claim the catalogue's own checks cannot make.
    pub fixture: String,
}

/// What must hold before a plugin is installed.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Proof {
    /// Unique within the plugin. What a verdict is reported against.
    pub id: String,
    /// What it establishes, in one line.
    pub title: String,
    /// Method and path.
    pub request: Request,
    /// What the answer must be.
    pub expect: Expect,
    /// A recorded response to run against where no instance exists.
    #[serde(default)]
    pub fixture: Option<String>,
    /// Which of this plugin's services is asked.
    ///
    /// Optional where the plugin declares one, required where it declares more. A proof
    /// gates the install of every service the plugin brings, and one that did not say
    /// which it was about would be checked against whichever the reader reached first —
    /// including its digest, which is what ties a recording to the image it came out of.
    #[serde(default)]
    pub service: Option<String>,
    /// Why this is worth asserting. A proof nobody can justify is one nobody will
    /// maintain.
    pub why: String,
}

/// A row in a register lemonfiber already runs.
///
/// The fields beyond `at` and `id` are the row its point declares, and the point is
/// published — so the required set, the optional set, the closed sets and the bounds
/// are read from `extension-points.json` rather than restated here. What this type
/// fixes is that a contribution is declared in this block and nowhere else, and that
/// it carries nothing outside the union of the rows the published points take.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Contribution {
    /// A point the build publishes. One it does not is refused by name.
    pub at: String,
    /// Namespaced with the declaring plugin's id, always.
    pub id: String,
    /// The one-line summary of what was checked.
    #[serde(default)]
    pub title: Option<String>,
    /// Which family a check is narrowed to.
    #[serde(default)]
    pub category: Option<String>,
    /// Method and path on the plugin's own service.
    ///
    /// There is no field for a host, so a check cannot be pointed at another service,
    /// at the machine, or off it. A plugin wanting to say something about a service it
    /// did not install is asking to speak for somebody else's software.
    #[serde(default)]
    pub request: Option<Request>,
    /// What the answer must be.
    #[serde(default)]
    pub expect: Option<Expect>,
    /// Why this is worth checking.
    #[serde(default)]
    pub why: Option<String>,
    /// The recorded response the check is proved against.
    #[serde(default)]
    pub fixture: Option<String>,
    /// A recorded response the check must fail on.
    ///
    /// For a check whose passing state cannot be recorded, such as one that needs an
    /// account nobody holds: the state it exists to find can be, and proving the check
    /// holds it to firing there. It may name the same file as `fixture`.
    #[serde(default)]
    pub fires_on: Option<String>,
    /// How long a check may run, within the bounds the point declares.
    #[serde(default)]
    pub timeout_s: Option<u32>,
    /// Which service the finding is about. Defaults to the plugin's own.
    #[serde(default)]
    pub service: Option<String>,
    /// The check a remedy is for, which must be one this same plugin declared.
    #[serde(default, rename = "for")]
    pub about: Option<String>,
    /// What to do, in the imperative.
    #[serde(default)]
    pub action: Option<String>,
    /// The technical half of a remedy, which must not lead.
    #[serde(default)]
    pub detail: Option<String>,
}

/// What is asked, and where.
///
/// Three fields and no more, and the third is the one worth explaining. A service that
/// answers XML unless a caller asks for JSON cannot satisfy a capability whose probe
/// requires a JSON assertion, and until this field there was nowhere to ask: Plex
/// answers `text/xml` at every path, including the one its health probe uses, unless
/// the request carries `Accept: application/json`.
///
/// **It is one media type and not a header map, and the difference is the point.** A
/// probe declares who it is asked as, and `none` on every `guarded` probe has meant what
/// it says partly because nothing could be presented. A map of headers would make that a
/// convention a reviewer has to hold — any service may name its credential header
/// whatever it likes, so no list of refused names could ever be closed — where one named
/// field keeps it a property of the format. A probe still cannot present anything,
/// because there is nowhere to write it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginRequest")]
pub struct Request {
    /// The HTTP method.
    pub method: String,
    /// The path on the service being asked.
    pub path: String,
    /// The one representation the answer is asked for, as a media type.
    ///
    /// Absent where the service needs no asking, which is most of them.
    #[serde(default)]
    pub accept: Option<String>,
}

/// What the answer has to be.
///
/// A status is a claim about the network path rather than about the service: Docker
/// publishes a port by putting a proxy in front of it, and that proxy accepts a
/// connection before knowing whether anything inside is listening. So a status alone
/// is not evidence — except for a refusal, which is the one answer no port proxy can
/// produce.
///
/// **A key of the four key-wise constraints is a place rather than a name.** A plain
/// name is a top-level member, and one beginning with `/` is a JSON Pointer, extended
/// with a step that picks an entry of a list by a field it holds. A flat name was enough while every service answered a flat object, and the
/// only thing it could say about a service that nests its payload was that the envelope
/// was there — which is a probe that passes by observing that something replied.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Expect {
    /// The status the answer must carry.
    #[serde(default)]
    pub status: Option<u16>,
    /// Places the answer must carry, each with the exact value it must hold.
    #[serde(default)]
    pub json: Option<BTreeMap<String, Expected>>,
    /// Places the answer must carry, whatever they hold.
    #[serde(default)]
    pub json_has_keys: Option<Vec<String>>,
    /// Places the answer must carry, each with the kind of value it must be.
    #[serde(default)]
    pub json_types: Option<BTreeMap<String, ExpectedKind>>,
    /// Places the answer must carry, each with a number it must not be below.
    #[serde(default)]
    pub json_at_least: Option<BTreeMap<String, i64>>,
    /// The answer read as an array, with at least this many entries.
    ///
    /// A catalogue is very often a list rather than an object, and none of the
    /// object-shaped constraints can say anything about one.
    #[serde(default)]
    pub json_array_min: Option<u64>,
    /// The answer did not parse as JSON at all.
    ///
    /// Which is what a service that serves its application shell for every path it does
    /// not implement answers — and the reason a status alone proves nothing against
    /// one: the shell comes back `200` whether the API behind it exists or not, so what
    /// has to be said is that the body was *not* a document.
    #[serde(default)]
    pub json_is_absent: Option<bool>,
    /// A substring of the content type the answer was served as.
    #[serde(default)]
    pub content_type: Option<String>,
    /// What the body must begin with, where it is not JSON.
    #[serde(default)]
    pub body_starts_with: Option<String>,
}

/// The kind of value a key must hold.
///
/// Five, and closed. A name outside them is one no runner could evaluate, and an
/// assertion nothing evaluates is a proof that silently checks less than it says —
/// which is worse than one that fails.
///
/// Named for what it is the kind *of*, rather than `Kind`, because this is published
/// and whoever generates from it flattens every definition into one scope. `Kind` is
/// the one name there a generator is certain to want for itself: every envelope this
/// contract describes is keyed by its `kind`, so the union of them is a `Kind` too, and
/// two of them in one module is a definition nothing can be compiled against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ExpectedKind {
    /// A true or a false.
    Bool,
    /// A whole number.
    Int,
    /// A string.
    Str,
    /// An array.
    List,
    /// An object.
    Dict,
}

/// Exactly what a place must hold.
///
/// Three kinds and no nesting: a flag that must be set, a number that must match, or a
/// word. A value deeper than this is asking about a document rather than about a claim
/// — and where the thing worth asserting is deeper *in* the answer, the key reaches it
/// rather than the value growing to match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum Expected {
    /// A true or a false.
    Flag(bool),
    /// A whole number.
    Number(i64),
    /// A word.
    Word(String),
}
