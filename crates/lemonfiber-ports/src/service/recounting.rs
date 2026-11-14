//! A filler telling the core what happened, so the core can tell the household.
//!
//! The core hands a curator or the media server its own address, the digest of the
//! certificate it presents there, a key minted for that one filler and how often to look;
//! from then on the filler sends each happening there as one neutral event, naming the
//! title and never a member, an account or what anybody is watching.

use std::ops::RangeInclusive;

use async_trait::async_trait;

use super::Failure;

/// The seconds apart an adapter that polls looks where nothing says otherwise.
pub const EVERY_DEFAULT: u32 = 60;

/// The seconds apart an adapter may be asked to look: no closer than a quarter of a
/// minute, and no further apart than an hour.
pub const EVERY: RangeInclusive<u32> = 15..=3600;

/// What happened, as a service tells it.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "NotifyEvent")]
pub struct Event {
    /// Which of the four happenings it is.
    pub event: Happened,
    /// The title it is about, by the service's own reference, where it is about one.
    #[serde(default)]
    pub reference: Option<String>,
    /// The title's name, or the service's own words for a problem about itself.
    pub name: String,
    /// What the service said went wrong, where something did.
    #[serde(default)]
    pub reason: Option<String>,
    /// Whether a problem the service raised about itself is cleared.
    #[serde(default)]
    pub cleared: bool,
}

/// One of the four happenings a service tells the core.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "NotifyHappened")]
pub enum Happened {
    /// A title a curator fetched is imported into the library.
    Imported,
    /// A grab or an import failed.
    Failed,
    /// The service raised a problem about itself, or cleared one.
    Health,
    /// A title was added to a library of the media server.
    Added,
}

/// The way an adapter hears what its upstream did.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum Hearing {
    /// The upstream's own notification connection tells it.
    Push,
    /// It reads the upstream's own history or activity, every so often.
    Poll,
}

/// How an adapter asked to tell the core what happened hears its upstream.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct Heard {
    /// Push or poll.
    pub hearing: Hearing,
    /// Where it polls, every how many seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub every: Option<u32>,
}

/// Asking a filler to tell the core what happened.
#[async_trait]
pub trait Recounting: Send + Sync {
    /// Send each happening to `address` with `key` from now on, refusing any connection
    /// there presenting a certificate whose SHA-256 is not `pin` and polling every `every`
    /// seconds where it polls, replacing whatever it was sending to before; with none of
    /// address, pin and key, send nothing more. Answers how it hears, or nothing where it
    /// stopped.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the filler is unreachable or refuses.
    async fn events(
        &self,
        address: Option<&str>,
        pin: Option<&str>,
        key: Option<&str>,
        every: Option<u32>,
    ) -> Result<Option<Heard>, Failure>;
}
