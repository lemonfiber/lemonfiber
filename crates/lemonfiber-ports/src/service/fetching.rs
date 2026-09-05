//! Stopping a download client fetching, and letting it fetch again.
//!
//! The third thing this product asks a download client to *do* rather than to say,
//! and deliberately apart from [`super::Throttling`]: how fast a client may go and
//! whether it goes at all are different questions, and a client held to a crawl is
//! not a client that has stopped. Folding the two together is how a word like
//! "pause" comes to name a very slow download.
//!
//! **Stopped means nothing new either.** A client whose current transfers were
//! halted and which starts the next thing handed to it has not stopped fetching —
//! it has stopped the things that were already running, and a request service will
//! hand it another within the hour. So a client is only stopped once nothing is
//! moving *and* nothing new would start, and each client answers for both halves in
//! its own dialect.
//!
//! **Only what was stopped here is started again.** That rule lives above this
//! port, in whatever decides to call these; what matters here is that starting and
//! stopping are two named requests rather than one flag, so a caller cannot lift an
//! operator's own pause by writing `false` where it meant "leave it alone".

use async_trait::async_trait;

use super::Failure;

/// Whether a download client is fetching at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pulling {
    /// It is fetching, or would start on the next thing handed to it.
    Fetching,
    /// Nothing is moving and nothing new would start.
    Stopped,
}

impl Pulling {
    /// Which of the two a client's two answers come to.
    ///
    /// Both halves, because either one alone is a client that goes on fetching:
    /// stopping what is running leaves the next grab to start, and refusing new
    /// work leaves what is already running to finish the month's allowance off.
    #[must_use]
    pub const fn of(moving: bool, would_start: bool) -> Self {
        if moving || would_start {
            Self::Fetching
        } else {
            Self::Stopped
        }
    }
}

/// Stopping a download client fetching, and starting it again.
#[async_trait]
pub trait Fetching: Send + Sync {
    /// Whether the client is fetching at all right now.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the client is unreachable or refuses.
    async fn pulling(&self) -> Result<Pulling, Failure>;

    /// Stop it fetching, and answer with what it reports afterwards.
    ///
    /// The answer is the read-back rather than an echo, for the reason
    /// [`super::Throttling::restrain`]'s is: a client that took the request and did
    /// not act on it looks exactly like one that did, from out here — and this is
    /// the request whose whole purpose is that a bill stops growing.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the client is unreachable, refuses, or cannot be
    /// read afterwards.
    async fn stop(&self) -> Result<Pulling, Failure>;

    /// Let it fetch again, and answer with what it reports afterwards.
    ///
    /// # Errors
    ///
    /// Returns [`Failure`] when the client is unreachable, refuses, or cannot be
    /// read afterwards.
    async fn resume(&self) -> Result<Pulling, Failure>;
}

#[cfg(test)]
mod tests {
    use super::Pulling;

    #[test]
    fn a_client_is_stopped_only_when_nothing_moves_and_nothing_new_would_start() {
        // Either half alone is a client that goes on fetching: stopping what is
        // running leaves the next grab to start, and refusing new work leaves what
        // is already running to spend the rest of the month.
        assert_eq!(Pulling::of(false, false), Pulling::Stopped);
        assert_eq!(Pulling::of(true, false), Pulling::Fetching);
        assert_eq!(Pulling::of(false, true), Pulling::Fetching);
        assert_eq!(Pulling::of(true, true), Pulling::Fetching);
    }
}
