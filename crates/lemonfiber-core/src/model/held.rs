//! What one member can actually watch, as the media server answers it for them.
//!
//! The household read says who is here and what each has *asked for*. This says what
//! is already there — the other half of the question a member opens the app with, and
//! the one nothing in this product could answer before.
//!
//! **Not a library listing.** The libraries read names the containers the server keeps;
//! this names what is inside them, and only what is inside them *for this member*. The
//! two are different questions and the second is not the first with a filter applied:
//! the age limit, the blocked kinds and which libraries an account may reach live on
//! the server, and it applies all three before it answers. Nothing here re-applies
//! them, so there is no second copy to disagree with the server on the day one moves.

use serde::Serialize;

/// The item and its kind, as the port that reads them defines them.
///
/// Re-exported rather than restated. A facing copy of the same four fields would be a
/// second shape for one thing, and the day one of them grew a field would be the day
/// the two stopped agreeing about what a household holds.
pub use crate::ports::service::{Held, Medium};

/// What one member can watch, and who they are.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct HeldReport {
    /// The member this was asked for, by the name they are known by.
    pub member: String,
    /// The identifier the media server files them under.
    pub id: String,
    /// What they hold, newest first.
    pub holdings: Vec<Held>,
    /// Whether the shelf could be read at all.
    ///
    /// An empty shelf and an unread one are different answers, and collapsing them
    /// would tell a household they own nothing on the day the media server rebooted.
    /// Anything that could not be read is said in `findings` and this goes false.
    pub available: bool,
    /// What is worth saying about this shelf, in the words its reader would use.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub findings: Vec<String>,
}
