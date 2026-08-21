//! Somewhere an alert can be sent.
//!
//! One method, because a channel has one job. What varies is where it goes and how it
//! fails, and both of those belong to the adapter rather than here.
//!
//! Delivery is deliberately allowed to fail. A channel that is unreachable is frequently
//! the same outage the operator needed telling about, so failing is an ordinary outcome to
//! be reported rather than an exception to be avoided — and nothing above this treats a
//! refusal as a reason to lose the alert.
//!
//! Here rather than in `lemonfiber-ports` with the other ports, and deliberately. It
//! delivers a [`crate::alert::Digest`], which is what an alert *is* — and that model reads
//! a service's health, which reads the compose model, none of which is vocabulary the
//! boundary should carry. A port with no adapter and no fake is a design note rather than a
//! seam anything crosses; when a channel is actually implemented, the question of what it
//! is handed can be settled then, and the port can move down with its answer.

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
