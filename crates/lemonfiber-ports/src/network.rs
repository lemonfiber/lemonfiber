//! Asking this machine about itself as a place on a network.
//!
//! Two questions, both about one particular machine: what it calls itself, and which
//! of a set of ports something on it is already answering on.
//!
//! Behind a port because they are the facts a test can never settle for itself: every
//! machine that runs these tests answers differently, so a test written against the
//! real ones would pass where it was written and nowhere else.
//!
//! Neither fails. A machine that will not say its name and one that has no name to say
//! are the same absence to whoever asked, and an error here would make the caller
//! decide twice — once for the failure and once for the absence — about one thing. The
//! same holds for what is listening: a caller that could not look and a caller that
//! looked and found nothing both go on to do exactly what they would have done.
//!
//! See `.docs/architecture/ports-and-adapters.md`.

use async_trait::async_trait;

/// Asks this machine about itself.
#[async_trait]
pub trait Site: Send + Sync {
    /// What this machine calls itself, read now rather than remembered, or nothing
    /// where it will not say.
    async fn name(&self) -> Option<String>;

    /// Which of these host ports something on this machine is already answering on.
    ///
    /// Asked about a set the caller names rather than listed wholesale, because
    /// everything listening on somebody's machine is a great deal of their business to
    /// carry around for the sake of a handful of numbers, and because the answer is
    /// only ever read against ports that were already wanted.
    ///
    /// Empty where nothing holds them, and empty where the machine would not say.
    async fn answering_on(&self, ports: &[u16]) -> Vec<u16>;
}
