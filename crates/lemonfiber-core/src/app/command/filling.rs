//! What is being asked about the stack's wiring.
//!
//! Its own value beside the other request shapes, for the reason each of those is:
//! the read and the one verb are two things asked of one subject, and a dispatcher
//! that split them at the top would put the listing and the change in different
//! parts of the same list.

/// Reading what reaches what, or changing one of those links.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Linking {
    /// Say what this stack wires to what, and how each link was settled.
    Read,
    /// Choose which service fills a capability.
    Fill(Filling),
}

/// One capability, and the service chosen to fill it.
///
/// The whole of what substituting takes. There is nothing here about *which* service
/// is being replaced, deliberately: a link asks for a capability and what answers it
/// is a setting, so naming the outgoing service would be asking the operator for a
/// fact lemonfiber already holds — and one they could get wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filling {
    /// The capability whose filler changes.
    pub capability: String,
    /// The service to fill it.
    pub service: String,
}
