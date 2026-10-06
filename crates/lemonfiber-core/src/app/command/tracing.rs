//! Following one item across the services.
//!
//! Its own file beside the command that carries it, the way every other value a
//! command carries has one.

/// Following one item across the services and reporting where it is, and whether
/// the indexers may be asked about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tracing {
    /// The show, film, or request to follow.
    pub term: String,
    /// The season to narrow the per-part coverage to, or every season where absent.
    pub season: Option<u32>,
    /// Whether the indexers may be asked what they carry for it.
    ///
    /// The one read in a trace that costs something outside this machine: it
    /// spends a live search against the daily allowance the indexers hold the
    /// operator to. Without it a trace that finds an item wanted and never
    /// grabbed cannot say whether the indexers carry nothing or the quality in
    /// force wants none of what they carry, and it says so rather than picking
    /// one. Carried like a diagnosis's widening ([`super::Diagnosing`]), and asked for at the
    /// door changes are asked for.
    pub searching: bool,
}
