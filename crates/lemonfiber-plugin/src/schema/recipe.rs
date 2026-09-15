//! The ordered calls that configure what a plugin installed.
//!
//! Declarable before anything runs one, and the manifest says so rather than implying
//! it: running a recipe is a capability a manifest asks for by name, so a lemonfiber
//! that does not offer it refuses such a manifest by naming that capability. Parsing
//! the block and skipping it would install a plugin whose declared behaviour is wider
//! than its actual one.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::Expect;

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
    /// A service id in this stack, or a DNS name outside it. Never an address.
    pub to: String,
    /// The path on it.
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
    /// Where in the answer it is read from.
    pub from: String,
    /// Whose value it is.
    pub origin: String,
}

/// One value, and the one destination it may reach.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Pair {
    /// The captured value, by the name the capture gave it.
    pub value: String,
    /// Where it may be carried.
    pub to: String,
}
