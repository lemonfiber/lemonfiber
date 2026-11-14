//! What an upgrade run answers with.
//!
//! One of the report families the machine-readable contract is made of; they live in
//! separate files and are re-exported as one, so `crate::model::X` reads the same as it
//! always did.

use serde::Serialize;

/// What became of asking one service to re-search its existing content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum Triggered {
    /// The re-search was accepted and now runs in the service's background.
    Started,
    /// The service had not finished starting — no key yet — so nothing was asked of
    /// it; running the upgrade again once it is up will reach it.
    NotStarted,
    /// The service refused the command or could not be reached.
    Failed {
        /// The service's own account of why.
        detail: String,
    },
}

/// What upgrading one kind of media costs in total, or why that could not be worked out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Total {
    /// The bytes re-fetching what its curator holds below the cutoff would take.
    Bytes(u64),
    /// Why the total could not be worked out, in the operator's words.
    Unknown(String),
}

/// One media type an upgrade covers: its chosen quality, that quality's cost, and —
/// once confirmed — what became of asking its service to re-search.
///
/// Reported per media type rather than as one figure, because each type carries its
/// own preset and so its own cost: film at maximum and television at space-saving are
/// upgraded to different bars, and a single number would misstate one of them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct UpgradeMedia {
    /// The media type: `tv`, `movies` or `music`.
    pub media_type: String,
    /// The preset in force for it, or for music the format.
    pub preset: String,
    /// What upgrading it costs in total: the runtime its curator holds below the cutoff
    /// times the bytes an hour takes at that preset or format.
    pub total: Total,
    /// What became of the re-search, or `None` where the upgrade was not confirmed
    /// and only the cost was stated.
    pub outcome: Option<Triggered>,
}

/// What upgrading existing content did, or — unconfirmed — would do.
///
/// Upgrading re-acquires the existing library at the chosen quality, which is a
/// large, bandwidth-expensive operation, so it is a separate explicit action whose
/// cost is stated before it runs and which does nothing until confirmed. Each curator
/// re-searches against its own current cutoff, so the report speaks per media type
/// rather than asserting one preset across the library.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct UpgradeReport {
    /// Whether the operator confirmed; without it nothing was triggered, only the
    /// cost stated.
    pub confirmed: bool,
    /// Per media type: its preset, that preset's cost, and — confirmed — the outcome.
    pub media: Vec<UpgradeMedia>,
    /// Whether this was a rehearsal: what would have happened, with none of it done.
    ///
    /// Said in a field of its own so that a rehearsal is never told from the real run by
    /// its wording alone.
    pub rehearsed: bool,
}
