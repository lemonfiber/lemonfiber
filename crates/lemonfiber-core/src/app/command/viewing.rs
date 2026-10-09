//! What a member asks for when they sit down to watch.

use super::Whom;
use crate::ports::service::HowFar;

/// What a member asks of the media server through the core: one title, what they were
/// part-way through, a grant for a device to play on, and how far they got.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Viewing {
    /// What one title is, as the member may see it.
    Title {
        /// Whose shelf it is read from.
        member: Whom,
        /// The title, by the identifier the shelf lists it under.
        id: String,
    },
    /// What a member was part-way through, most recent first.
    PartWay {
        /// Whose it is.
        member: Whom,
        /// How many to answer with.
        most: u32,
    },
    /// Open a session on a member's own account for one of their devices.
    Grant {
        /// The member the device plays as, by name or id.
        member: String,
        /// The id the player keeps for the device it plays on.
        device: String,
    },
    /// Record how far through one title a member is.
    Watched {
        /// The member, by name or id.
        member: String,
        /// The title or episode, by the identifier the shelf lists it under.
        id: String,
        /// How far in, and whether they finished it.
        how_far: HowFar,
    },
}
