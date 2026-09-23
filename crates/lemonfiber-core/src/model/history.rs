//! What the record of changes answers with.
//!
//! One of the report families the machine-readable contract is made of; they live in
//! separate files and are re-exported as one, so `crate::model::X` reads the same as it
//! always did.

use serde::Serialize;

/// One change lemonfiber made, and whether it could be put back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct ChangeReport {
    /// When it was made, as whole seconds since the Unix epoch, written in decimal.
    ///
    /// A string of digits rather than a number, because it is the stamp the record keeps
    /// and a stamp is compared and stored as text; what it counts is stated here so a
    /// reader can turn it into a time without guessing at a format.
    ///
    /// **`0` means the clock was unreadable when the change was written**, not that it
    /// was made at the epoch: it is how a machine whose clock would not answer stamps a
    /// change. It is not an instant, so two changes both stamped `0` were not made at
    /// the same moment, and a reader showing it as a date in 1970 would be inventing
    /// one.
    #[schemars(regex(pattern = r"^[0-9]+$"))]
    pub at: String,
    /// The operation that made it — a seed, a reconfigure, an applied fix — so a
    /// history reads as what happened rather than as bare diffs.
    pub operation: String,
    /// What it was made to.
    pub target: String,
    /// What it did, in the operator's terms.
    pub did: String,
    /// How far it could be put back.
    pub reversal: crate::rollback::Reversal,
    /// Why it could not go further, where it could not.
    pub because: Option<String>,
    /// What to do instead, where there is something.
    pub instead: Option<String>,
    /// How many changes that one operation made, this one among them.
    ///
    /// An operation is the unit an operator agreed to, and undoing half of one leaves a
    /// machine in a state nobody chose — so what a single line would take with it is on
    /// the line rather than left to be counted off the list.
    pub alongside: usize,
}

/// Everything lemonfiber changed, most recent first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct HistoryReport {
    /// The changes, newest first.
    pub changes: Vec<ChangeReport>,
    /// How far back the record goes, in the operator's terms.
    ///
    /// Stated rather than left to be inferred from the oldest entry: a record that has
    /// been trimmed and one that has always been short look identical from the entries
    /// alone, and only one of them means something is missing.
    pub horizon: String,
}
