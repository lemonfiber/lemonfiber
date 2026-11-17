//! A media server's live TV, asked for one member: the channels their own account may
//! watch, each one's guide across a number of hours, opening one channel's live stream on
//! their account, and closing it.
//!
//! Every answer is the member's own account's, never what the adapter's credential may
//! watch, so a member whose account may not watch live TV is answered no channel and
//! nothing is opened for them.

use async_trait::async_trait;

use super::Failure;
use crate::instant::Instant;

/// One live TV channel, as one member's account may watch it.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Channel {
    /// The identifier the server tells it apart by.
    pub id: String,
    /// What it is called.
    pub name: String,
    /// Its number, where it has one.
    pub number: Option<String>,
    /// The programme on it now, where the server knows one.
    pub now: Option<Programme>,
}

/// A programme on a channel.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Programme {
    /// What it is called.
    pub title: String,
    /// Its certificate, as the server holds it, where it has one.
    pub certificate: Option<String>,
    /// When it ends, where the server knows.
    #[serde(
        default,
        deserialize_with = "crate::instant::until_adapters_answer_integers"
    )]
    pub ends: Option<Instant>,
}

/// The hours a guide may be asked across: one at least, a week at most.
pub const GUIDE_HOURS: std::ops::RangeInclusive<u32> = 1..=168;

/// One channel's guide, as one member's account may watch it: the programmes on it from
/// now across the hours asked, the one on now first.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Listing {
    /// The channel, by the identifier the channels list gives it.
    pub channel: String,
    /// Each programme the server's guide holds on it from now across the hours asked, in
    /// the order they air, none past where the guide ends, and from minor 6 none the
    /// server's own policy keeps from the member's account.
    pub programmes: Vec<Airing>,
    /// From minor 6, where the server's guide ends on it across the hours asked: the end of
    /// the last programme it holds there before any is left out, none where it holds none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<Instant>,
}

/// A programme in a channel's guide.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Airing {
    /// What it is called.
    pub title: String,
    /// Its certificate, as the server holds it, where it has one.
    pub certificate: Option<String>,
    /// When it starts.
    pub starts: Instant,
    /// When it ends.
    pub ends: Instant,
}

/// A channel's live stream, opened on one member's account.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Tuned {
    /// The id it is closed by.
    pub closes_by: String,
    /// The path and query at the upstream of the tuner's own stream.
    pub direct: String,
    /// The path and query at the upstream of a transcoded playlist of it, naming the live
    /// stream in its query.
    pub transcoded: String,
}

/// A media server asked about live TV for one member.
#[async_trait]
pub trait Tuning: Send + Sync {
    /// The live TV channels `member`'s own account may watch, each with the programme on
    /// it now; none where their account may not watch live TV.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses, and
    /// [`Failure::Unsupported`] where it has no live TV.
    async fn channels(&self, member: &str) -> Result<Vec<Channel>, Failure>;

    /// The guide of each channel `member`'s own account may watch, across `hours` from
    /// now, and where the server's guide ends on it; none where their account may not watch
    /// live TV.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses, and
    /// [`Failure::Unsupported`] where it has no live TV.
    async fn guide(&self, member: &str, hours: u32) -> Result<Vec<Listing>, Failure>;

    /// Open `channel`'s live stream on `member`'s own account, where it may watch live TV
    /// and that channel; nothing where it may not.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses, and
    /// [`Failure::Unsupported`] where it has no live TV.
    async fn tune(&self, member: &str, channel: &str) -> Result<Option<Tuned>, Failure>;

    /// Close the live stream `tune` answered `closes_by` for.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the server is unreachable or refuses, and
    /// [`Failure::Unsupported`] where it has no live TV.
    async fn untune(&self, closes_by: &str) -> Result<(), Failure>;
}
