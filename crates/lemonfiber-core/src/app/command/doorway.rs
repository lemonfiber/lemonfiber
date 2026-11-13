//! Who asks for the front door, and from where.

use std::net::IpAddr;

/// Who asks for the front door, and from where.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Doorway {
    /// The address the request came from; none for this machine itself.
    pub from: Option<IpAddr>,
    /// Whether it is answered for a household member.
    pub household: bool,
}
