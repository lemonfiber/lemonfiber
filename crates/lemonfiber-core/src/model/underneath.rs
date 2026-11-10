//! What a read of how the stack runs without lemonfiber answers with.
//!
//! One of the report families the machine-readable contract is made of; they live in
//! separate files and are re-exported as one, so `crate::model::X` reads the same as it
//! always did.

use serde::Serialize;

/// Which stack this machine runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum StackSource {
    /// The stack this build ships.
    Bundled,
    /// The operator's own stack directory, by its path.
    Own(String),
}

/// How any action is run without a question being put.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct NonInteractive {
    /// The words an action is run with, as the command line takes them.
    pub form: String,
}

/// A part of the stack that the plain Compose words alone do not bring up as lemonfiber
/// does, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct StackException {
    /// The part, by its id.
    pub part: String,
    /// Why, in the operator's words.
    pub because: String,
}

/// How this stack runs without lemonfiber.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct StackReport {
    /// Whether this machine runs the stack this build ships or the operator's own.
    pub source: StackSource,
    /// The directory the Compose project is written to.
    pub project: String,
    /// The `.env` file beside it.
    pub env: String,
    /// The `docker compose` words that bring the stack up with no lemonfiber running, as
    /// lemonfiber's own start would run them.
    pub plain_up: Vec<String>,
    /// How any action is run without a question being put.
    pub non_interactive: NonInteractive,
    /// Every part the words alone do not bring up as lemonfiber does. Empty where there
    /// is none.
    pub exceptions: Vec<StackException>,
}
