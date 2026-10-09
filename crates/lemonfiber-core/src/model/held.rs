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

pub use crate::ports::service::Medium;

/// One thing the household holds, as a member is shown it.
///
/// What a person recognises and nothing else. There is no file path, no container,
/// no bitrate and no library id: a member deciding what to watch is not choosing a
/// transcode, and a surface handed those would have to decide not to draw them.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct Held {
    /// The identifier the server tells it apart by, which is what asking to play one
    /// of them names.
    pub id: String,
    /// What it is called, in the words the server holds it under.
    pub title: String,
    /// The year it came out, where the server knows one. Absent rather than guessed:
    /// two films share a title far more often than they share a title and a year.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<u16>,
    /// Which of the kinds this product deals in it is.
    pub medium: Medium,
    /// Where it is served at the guarded front door, or why it is not.
    #[serde(flatten)]
    pub at: Located,
}

/// Where one item is served at the guarded front door, or why it is not.
///
/// Every location is built by the core from the household address the stack publishes
/// for the media server, so a client never puts one together.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct Located {
    /// Where its poster is served, where it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poster: Option<String>,
    /// Where its backdrop is served, where it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backdrop: Option<String>,
    /// Where it streams from, where it plays.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_from: Option<String>,
    /// The certificate the door presents, which a client pins, beside any location.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub door: Option<Pinned>,
    /// Why no location is stated, where none is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unlocated: Option<String>,
}

/// The certificate a guarded door presents, as a client pins it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct Pinned {
    /// SHA-256 over the certificate's DER encoding, lower-case hex.
    pub fingerprint: String,
}

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
    ///
    /// Always written, empty or not. A field the schema requires and the document
    /// sometimes omits is one a reader has to guess about, and an empty list already
    /// says the thing it would say: there is nothing to report about this shelf.
    pub findings: Vec<String>,
    /// Whether this was a rehearsal: what would have happened, with none of it done.
    ///
    /// Said in a field of its own so that a rehearsal is never told from the real run by
    /// its wording alone.
    pub rehearsed: bool,
}
