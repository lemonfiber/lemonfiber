//! The ordered calls that configure what a plugin installed.
//!
//! Running a recipe is a capability a manifest asks for by name, so a lemonfiber that
//! does not offer it refuses such a manifest by naming that capability. Parsing the
//! block and skipping it would install a plugin whose declared behaviour is wider than
//! its actual one.
//!
//! **Bounded by its shape.** A step is made at most once, in the order written: a guard
//! can skip one and nothing can jump, and a retry is bounded by numbers this format
//! publishes. So the set of things a recipe could do is computable by reading it.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::Expect;

/// The capability a manifest asks for in order to declare a recipe.
///
/// Named beside the block that asks for it rather than in the register of what this
/// build offers, so that it stays a fact about recipes: the register answers whether an
/// offer is made, and the name of the thing being asked for belongs with the thing.
pub const RUN: &str = "recipe.run";

/// How many times a step may be made again, at most.
pub const MOST_RETRIES: u32 = 10;

/// The longest a step may wait between two tries, in seconds.
pub const LONGEST_WAIT: u64 = 30;

/// The longest a recipe's retries may wait, in all, in seconds.
pub const ALL_WAITING: u64 = 5 * 60;

/// The longest one call may take, from asking to its last byte, in seconds.
pub const CALL_DEADLINE: u64 = 30;

/// The most one answer's body may hold, in bytes.
pub const LARGEST_ANSWER: usize = 1024 * 1024;

/// What a wait between two tries is written as, after its number.
const SECONDS: char = 's';

/// What a capture reading a header of the answer begins with, before the header's name.
pub const HEADER: &str = "header.";

/// One named flow of calls.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    /// Unique within the plugin.
    pub id: String,
    /// What the flow accomplishes, in one line.
    pub title: String,
    /// Why it is worth running.
    pub why: String,
    /// When it runs: as part of installing the plugin, or when it is asked for.
    #[serde(default)]
    pub on: On,
    /// The values it brings in from outside an answer: a credential lemonfiber holds,
    /// or one the operator supplies when it runs.
    #[serde(default, rename = "input")]
    pub inputs: Vec<Input>,
    /// The calls, in the order they are made.
    #[serde(default, rename = "step")]
    pub steps: Vec<Step>,
    /// Every value this flow could carry to every destination it could reach.
    ///
    /// A flow with no pair behind it fails validation before a call is made, which is
    /// what makes "what does this plugin tell whom" answerable from the manifest.
    #[serde(default, rename = "pair")]
    pub pairs: Vec<Pair>,
}

/// When a recipe runs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "PluginRecipeOn")]
pub enum On {
    /// During an install and an update, after the plugin's proofs and the stack's checks
    /// hold and before the install is recorded. What a recipe that says nothing does,
    /// because a first-run flow is what a recipe is for.
    #[default]
    Install,
    /// Only when the operator asks for it by name, with approvals of its own.
    Demand,
}

/// Where a value a recipe carries comes from.
///
/// Four, and no fifth: a service in this stack and a host outside it each answer a
/// call, and lemonfiber's credential store and the operator each supply a value no call
/// answered. Written by the manifest, and held to where the value really comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "PluginValueOrigin")]
pub enum Origin {
    /// The answer of a call to a service in this stack.
    StackService,
    /// The answer of a call to a host outside it.
    ExternalResponse,
    /// A credential lemonfiber already holds for a service.
    CredentialStore,
    /// A value the operator supplies when the recipe runs.
    Operator,
}

impl Origin {
    /// The word the manifest writes it as.
    #[must_use]
    pub const fn written(self) -> &'static str {
        match self {
            Self::StackService => "stack-service",
            Self::ExternalResponse => "external-response",
            Self::CredentialStore => "credential-store",
            Self::Operator => "operator",
        }
    }
}

/// A value a recipe brings in from outside an answer.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Input {
    /// What the value is called, for a call to name.
    pub name: String,
    /// Whose value it is: the credential store, or the operator.
    pub origin: Origin,
    /// The service whose credential it is, where the credential store holds it.
    #[serde(default)]
    pub of: Option<String>,
    /// What the operator is asked, in one line, where the operator supplies it.
    #[serde(default)]
    pub ask: Option<String>,
    /// Whether what the operator supplies may be a secret, so a terminal takes it without
    /// showing it.
    #[serde(default)]
    pub secret: bool,
}

/// One call in a flow.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Step {
    /// Unique within the recipe.
    pub id: String,
    /// What is called, and where.
    pub call: StepCall,
    /// What the answer must be for the flow to go on.
    #[serde(default)]
    pub expect: Option<Expect>,
    /// The values taken out of the answer, for a later call to substitute.
    #[serde(default)]
    pub capture: Vec<Capture>,
    /// What must hold for the step to be made; where it does not, the step is skipped.
    #[serde(default)]
    pub when: Option<Condition>,
    /// How often the step is made again before its answer is taken as final.
    #[serde(default)]
    pub retry: Option<Retry>,
}

/// What an earlier step or value must have come to.
///
/// One of two shapes, and a condition carrying neither or both is refused when the
/// manifest is read: an earlier step answered a status, or an earlier value holds a
/// text exactly.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginRecipeCondition")]
pub struct Condition {
    /// The earlier step whose answer is asked about.
    #[serde(default)]
    pub step: Option<String>,
    /// The status that step must have answered.
    #[serde(default)]
    pub status: Option<u16>,
    /// The earlier capture or input asked about.
    #[serde(default)]
    pub value: Option<String>,
    /// The text it must hold, exactly.
    #[serde(default)]
    pub equals: Option<String>,
}

/// Making a step again until its answer is the one wanted.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginRecipeRetry")]
pub struct Retry {
    /// How many times it is made again, at most: no more than [`MOST_RETRIES`].
    pub times: u32,
    /// How long it waits between two, written as whole seconds, `6s`, and no longer
    /// than [`LONGEST_WAIT`].
    pub every: String,
    /// What the answer must come to for it to stop: a status, or a value it captured.
    pub until: Condition,
}

impl Retry {
    /// How long it waits between two tries, in seconds, where `every` is written as the
    /// format allows: a whole number of seconds from 1 to [`LONGEST_WAIT`], then `s`.
    #[must_use]
    pub fn seconds(&self) -> Option<u64> {
        self.every
            .strip_suffix(SECONDS)
            .filter(|digits| !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()))
            .and_then(|digits| digits.parse().ok())
            .filter(|seconds| (1..=LONGEST_WAIT).contains(seconds))
    }
}

/// What one step calls.
///
/// `headers` and `body` are where an earlier capture is substituted, and they are the
/// whole of it. Without somewhere to put one, a recipe could name a destination and
/// capture a value and had no way to carry the one to the other — which is every
/// first-run flow there is: creating an account and reading back a token is worth
/// nothing if the token cannot then be presented.
///
/// They are also what makes the set of flows a recipe could produce computable by
/// reading it: every substitution inside a call is a flow from that value to that
/// call's destination, and a call with nowhere to substitute into produces none.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StepCall {
    /// The HTTP method.
    pub method: String,
    /// A service id in this stack, or a DNS name outside it. Never an address, and
    /// never a substitution.
    pub to: String,
    /// The path on it, written out. Never a substitution.
    pub path: String,
    /// Headers the call carries. A value may substitute an earlier capture.
    #[serde(default)]
    pub headers: Option<BTreeMap<String, String>>,
    /// The body the call carries. A value may substitute an earlier capture.
    #[serde(default)]
    pub body: Option<String>,
}

/// A value taken out of an answer.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Capture {
    /// What the value is called, for a later call to name.
    pub name: String,
    /// Where in the answer it is read from: an expectation key, or `header.<name>`.
    pub from: String,
    /// Whose value it is: the service in this stack or the host outside it that
    /// answered.
    pub origin: Origin,
}

/// One value, and the one destination it may reach.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Pair {
    /// The value, by the name its capture or its input gives it.
    pub value: String,
    /// Where it may be carried.
    pub to: String,
}

#[cfg(test)]
mod tests;
