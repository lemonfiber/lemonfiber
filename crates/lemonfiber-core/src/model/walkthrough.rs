//! What a guided walkthrough answers with.
//!
//! One of the report families the machine-readable contract is made of; they live in
//! separate files and are re-exported as one, so `crate::model::X` reads the same as it
//! always did.

use serde::Serialize;

/// What a first-content walkthrough did — the whole of it, narrated line by line as it
/// happened and gathered here so the ending can be rendered, serialised and exited on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct WalkthroughReport {
    /// Which walk this was.
    pub shape: crate::walkthrough::Shape,
    /// Where it ended up.
    pub state: crate::walkthrough::State,
    /// What it set out to prove, said so the operator knows what they watched.
    pub proves: String,
    /// What it walked, where it got as far as choosing something.
    pub item: Option<String>,
    /// Every line it said, in order — the same lines the operator watched arrive, kept so
    /// a machine-readable run is not a silent one.
    pub lines: Vec<crate::walkthrough::Line>,
    /// Where and why it stopped, where it did.
    pub stopped: Option<crate::walkthrough::Stopped>,
    /// What the import did with the file, where it got that far.
    pub link: Option<crate::walkthrough::Link>,
    /// Where it leaves the operator, where it worked.
    pub handover: Option<crate::walkthrough::Handover>,
    /// What could have been walked instead, where nothing was chosen — the safe first
    /// attempts, so an operator with an empty library is not left guessing.
    pub suggestions: Vec<String>,
    /// Whether the download was handed to the background rather than waited out.
    pub in_background: bool,
    /// Whether what was asked for was already here, and so was not acquired again.
    pub already_here: bool,
}
