//! Gathering what somebody helping would ask for.
//!
//! Its own file beside the command that carries it, the way every other value a
//! command carries has one.

use super::super::{bundle, support};

/// Whether to write the support file, what goes in it, and where it goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gathering {
    /// Whether to produce the file, rather than say what one would hold.
    pub write: bool,
    /// What goes in it, and what was agreed to going in it.
    pub wanted: bundle::Wanted,
    /// Where it is written, for a run that produces one.
    pub dest: support::Destination,
}
