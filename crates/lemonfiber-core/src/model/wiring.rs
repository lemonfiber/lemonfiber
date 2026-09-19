//! What reaches what in this stack, and what changing one of those links costs.
//!
//! One of the report families the machine-readable contract is made of; they live in
//! separate files and are re-exported as one, so `crate::model::X` reads the same as it
//! always did.
//!
//! The listing is the answer to a question nothing could be asked before: *what does
//! this stack wire to what, and would anything else do?* Every service declared what
//! it could do and nothing said what asked, so the answer lived in an ordering edge, a
//! Compose variable and a service id in this crate's own source — three statements of
//! one fact, none of which said what the link was for.
//!
//! The substitution is the other half. Choosing which service fills a capability is
//! the change, and what it would cost is worked out and reported before it is made,
//! because every answer here is only worth having in advance.

use serde::Serialize;

use crate::wiring::{Substitution, Unfilled, Wired};

/// What this stack wires to what.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct WiringReport {
    /// Every link, in the order the stack declares them.
    pub wired: Vec<Wired>,
    /// Every capability something asks for and nothing fills, naming what asked.
    ///
    /// Repeated out of the links above rather than left to be found among them: a
    /// stack with one unfilled ask among twenty working ones is a stack whose one
    /// problem is a line in a list, and a consumer that had to notice it would be
    /// the reason nobody did.
    pub unfilled: Vec<Unfilled>,
}

/// What substituting one service for another would come to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct SubstitutionReport {
    /// The change itself, and what it would leave with nothing filling it.
    pub substitution: Substitution,
    /// Whether it was written, or only worked out.
    ///
    /// A run that only says what it would do writes nothing and reports the same
    /// answer, so the two are told apart here rather than by the caller remembering
    /// which flags it passed.
    pub applied: bool,
}
