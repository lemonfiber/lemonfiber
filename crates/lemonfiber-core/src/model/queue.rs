//! What a stuck-queue reading answers with.
//!
//! One of the report families the machine-readable contract is made of; they live in
//! separate files and are re-exported as one, so `crate::model::X` reads the same as it
//! always did.

use serde::Serialize;

/// One stuck item queue health found, named so it links straight to its own trace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct StuckEntry {
    /// The item's title — the term a `trace` searches by.
    pub title: String,
    /// The \*arr whose queue is holding it.
    pub service: String,
    /// The stage its download is stuck at.
    pub stage: crate::trace::Stage,
}

/// The items whose downloads are stuck, across the \*arrs — the landing point for "N
/// items stuck" that queue health reports, each entry naming the item so the operator
/// goes straight to its per-item trace rather than to a count to investigate.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct StuckReport {
    /// The stuck items, each linkable to its trace.
    pub items: Vec<StuckEntry>,
    /// Whether an \*arr's queue could not be read, so the list may be short — reported
    /// rather than read as "nothing stuck", the same honesty a trace keeps.
    pub incomplete: bool,
}
