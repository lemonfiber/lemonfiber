//! What the notification surfaces answer with.
//!
//! One of the report families the machine-readable contract is made of; they live in
//! separate files and are re-exported as one, so `crate::model::X` reads the same as it
//! always did.

use serde::Serialize;

/// One event kind the operator set apart from the preset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct ExceptionReport {
    /// The kind of event, by the name a finding gives it.
    pub kind: String,
    /// Whether it is heard about, whatever the preset would say.
    pub wanted: bool,
}

/// What the operator will be told about, and what changing it came to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct AlertReport {
    /// The preset in force for events with no exception of their own.
    pub preset: String,
    /// What that preset means, in the operator's terms.
    pub means: String,
    /// Events set apart from the preset, quietest name first.
    pub exceptions: Vec<ExceptionReport>,
    /// Whether this call changed the answer.
    pub changed: bool,
    /// Whether it only reported what it would have written.
    pub rehearsed: bool,
}
