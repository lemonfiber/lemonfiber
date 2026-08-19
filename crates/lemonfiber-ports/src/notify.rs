//! Somewhere an alert can be sent.
//!
//! One method, because a channel has one job. What varies is where it goes and
//! how it fails, and both of those belong to the adapter rather than here.
//!
//! Delivery is deliberately allowed to fail. A channel that is unreachable is
//! frequently the same outage the operator needed telling about, so failing is an
//! ordinary outcome to be reported rather than an exception to be avoided — and
//! nothing above this treats a refusal as a reason to lose the alert.

use async_trait::async_trait;

use crate::alert::Digest;

/// A channel would not take it, in its own words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Undelivered {
    /// Which channel refused, so the operator is told which one to look at.
    pub channel: String,
    /// Why, as the channel put it.
    pub reason: String,
}

/// Somewhere alerts can be sent.
#[async_trait]
pub trait Channel: Send + Sync {
    /// What this channel is called, in the words an operator configured it by.
    fn name(&self) -> &str;

    /// Send a digest.
    ///
    /// # Errors
    ///
    /// Returns [`Undelivered`] where the channel could not be reached or refused
    /// what it was given. The caller records the failure and keeps the alert; a
    /// channel is never trusted to be the only copy.
    async fn deliver(&self, digest: &Digest) -> Result<(), Undelivered>;
}
