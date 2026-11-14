//! What a plugin says stops working where the operator refuses a host it reaches.

use serde::Deserialize;

/// What refusing one host a plugin's recipes reach stops, as its author says it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginReach")]
pub struct Reach {
    /// The host outside the stack, as a recipe's call or pair names it.
    pub host: String,
    /// What stops working where the operator refuses it, in one line.
    pub stops: String,
}
