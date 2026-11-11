//! A filler telling the core what happened, so the core can tell the household.
//!
//! The core hands a curator or the media server its own address, the digest of the
//! certificate it presents there, a key minted for that one filler and how often to look;
//! from then on the filler sends each happening there as one neutral event, naming the
//! title and never a member, an account or what anybody is watching.

use async_trait::async_trait;

use super::Failure;

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
