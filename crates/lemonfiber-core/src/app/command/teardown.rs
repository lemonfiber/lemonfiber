//! Stopping and removing what a form started.
//!
//! Its own file beside the command that carries it, the way every other value a
//! command carries has one.

use super::super::Waiting;

/// Stopping and removing what a form started, and whether to wait for downloads first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Teardown {
    /// The forms to stop.
    pub forms: Vec<String>,
    /// Whether anything still downloading is let finish before the stop.
    ///
    /// The wait is inside the command rather than in front of it, so a surface
    /// that cannot sit in a loop asks for it by saying so. Whether to offer the
    /// choice at all is the surface's — a terminal asks, a machine-readable run
    /// is not asked — but the waiting itself is one implementation.
    pub wait: Waiting,
}
