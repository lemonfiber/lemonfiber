//! What a surface asks of the core for itself.

use super::{Exporting, Fetching, Polled};

/// What a surface asks of the core for itself: what it composes its routes from, the
/// reads its streams poll, what it does on every request or on a timer, the bytes it
/// hands over as they are, and the export its log viewer writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Surfacing {
    /// What the web API composes its routes and its answers from.
    Standing,
    /// A read a stream polls.
    Polled(Polled),
    /// Hand on an event a service posted, with the key it presented.
    Told(crate::notify::events::Telling),
    /// Renew the grant to play of a member whose client spoke.
    Spoke {
        /// The member, by the id their session or key names.
        member: String,
    },
    /// Close the live streams whose passes lapsed unrenewed.
    Untune,
    /// Read bytes to hand over as they are: a picture, a kept support bundle.
    Fetch(Fetching),
    /// Write what the log viewer shows into a file of its own, redacted as a support
    /// bundle is.
    Export(Exporting),
}
