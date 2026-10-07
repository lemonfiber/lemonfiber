//! What the media server is playing now, and for whom it was asked.
//!
//! The household read is what has been asked for and the shelf is what is already
//! here. This is what is being watched at this moment: who, what, and on which device.
//! For the operator it is everybody; for a member it is their own sessions, because the
//! command that answers a member names that member and no other.

use serde::Serialize;

/// One session playing something, as the port that reads it defines it.
///
/// Re-exported rather than restated, so the port and the report cannot come to
/// disagree about what one session carries.
pub use crate::ports::service::Playback;

/// What is playing now, and whose sessions were asked about.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct PlayingReport {
    /// The member this was narrowed to, by the name they are known by, or empty where
    /// it is every session in the house.
    pub member: String,
    /// Every session playing something, in the order the media server lists them. How
    /// many are playing is how many these are, rather than a count kept beside them.
    pub sessions: Vec<Playback>,
    /// Whether the media server could be asked at all.
    ///
    /// Nobody watching and a server that would not say are different answers, and
    /// collapsing them would report a quiet house on the day the media server was down.
    /// Anything that could not be read is said in `findings` and this goes false.
    pub available: bool,
    /// What is worth saying about this reading, in the words its reader would use.
    ///
    /// Always written, empty or not, so a reader never has to guess what an absent
    /// field means.
    pub findings: Vec<String>,
}
