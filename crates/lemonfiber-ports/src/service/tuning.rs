//! A media server's live TV, asked for one member: the channels their own account may
//! watch, opening one channel's live stream on their account, and closing it.
//!
//! Every answer is the member's own account's, never what the adapter's credential may
//! watch, so a member whose account may not watch live TV is answered no channel and
//! nothing is opened for them.

use async_trait::async_trait;

use super::Failure;

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
    /// When it ends, in RFC 3339, where the server knows.
    pub ends: Option<String>,
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
