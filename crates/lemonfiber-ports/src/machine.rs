//! Asking this machine about the state it is in, rather than about its name.
//!
//! Three facts, one seam, and they belong together for the reason
//! [`crate::network`]'s does: each is a property of one particular machine at one
//! particular moment, so a test written against the real one would pass where it
//! was written and nowhere else. A laptop on a train and a server in a cupboard
//! answer differently, and the one that matters is always the machine that is not
//! here.
//!
//! None fails. A machine that will not say when it started and a machine with
//! nothing to say are the same absence to whoever asked, and an error here would
//! make the caller decide twice — once for the failure and once for the absence —
//! about one thing. What a caller does with the absence is its own business, and
//! the callers here answer it differently on purpose: not knowing when a machine
//! started means nothing has changed since last time, not knowing where its power
//! comes from means there is no battery to protect, and not knowing whether a
//! process is running means it is waited on as though it were.
//!
//! See `.docs/architecture/ports-and-adapters.md`.

use async_trait::async_trait;

/// Asks this machine when it last started.
#[async_trait]
pub trait Started: Send + Sync {
    /// The moment this machine last started, in seconds since the epoch, or nothing
    /// where it will not say.
    ///
    /// The moment rather than how long ago, because the answer is compared with one
    /// written down by an earlier run: an elapsed time is a different number every
    /// time it is read, and two of them cannot be compared at all.
    async fn at(&self) -> Option<u64>;
}

/// Where a machine's power is coming from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Power {
    /// Plugged in.
    Mains,
    /// Running off a battery.
    Battery,
}

/// Asks this machine where its power is coming from.
#[async_trait]
pub trait Supply: Send + Sync {
    /// Where the power is coming from now, or nothing where this machine will not
    /// say — which is what a desktop with no battery at all looks like.
    async fn source(&self) -> Option<Power>;
}

/// Asks this machine whether a process is still running.
#[async_trait]
pub trait Running: Send + Sync {
    /// Whether the process with this id is running on this machine now, or nothing
    /// where the machine will not say.
    ///
    /// A process id is reused once its process has gone, so `true` can be a different
    /// program under the same number. What that costs a caller is a wait it did not
    /// need, which is the side to be wrong on.
    async fn alive(&self, pid: u32) -> Option<bool>;
}
