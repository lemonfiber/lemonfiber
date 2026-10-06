//! Running the diagnostic checks.
//!
//! Its own file beside the command that carries it, the way every other value a
//! command carries has one.

use crate::doctor::Narrowing;

/// What a diagnosis is narrowed to, and what the operator opted into or answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnosing {
    /// What the run is narrowed to. A single check is named by the identifier
    /// its finding carries, so a report can be read and asked for again.
    pub narrowing: Narrowing,
    /// Whether the operator opted into the checks that disturb the system.
    pub disruptive: bool,
    /// A check whose warning the operator is answering: they have weighed the
    /// cost and chosen it, so it stops leading from now on.
    pub accept: Option<String>,
}
