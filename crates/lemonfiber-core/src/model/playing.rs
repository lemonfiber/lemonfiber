//! What the media server is playing now, and for whom it was asked.
//!
//! The household read is what has been asked for and the shelf is what is already
//! here. This is what is being watched at this moment: who, what, and on which device.
//! For the operator it is everybody; for a member it is their own sessions, because the
//! command that answers a member names that member and no other.

use serde::Serialize;

pub use crate::ports::service::Medium;

/// Somebody watching something now, as the media server lists the session.
///
/// Who and what, and where: what a household recognises about somebody watching. No
/// stream, no bitrate and no transcode reason, because a member is not choosing one and
/// a surface handed those would have to decide not to draw them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Playback {
    /// The identifier the server files the account under.
    pub member_id: String,
    /// The name the account is known by.
    pub member: String,
    /// What is playing, in the words the server holds it under: an episode's own name
    /// where it is an episode.
    pub title: String,
    /// The series an episode belongs to, where it is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series: Option<String>,
    /// The season an episode is in, where the server numbers one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub season: Option<u32>,
    /// The episode's number within its season, where the server numbers one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episode: Option<u32>,
    /// Which of the kinds this product deals in it is. An episode is of a series.
    pub medium: Medium,
    /// Whether it is paused rather than playing.
    pub paused: bool,
    /// What the device it plays on calls itself.
    pub device: String,
}

impl From<crate::ports::service::Playback> for Playback {
    fn from(session: crate::ports::service::Playback) -> Self {
        Self {
            member_id: session.member_id,
            member: session.member,
            title: session.title,
            series: session.series,
            season: session.season,
            episode: session.episode,
            medium: session.medium,
            paused: session.paused,
            device: session.device,
        }
    }
}

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
