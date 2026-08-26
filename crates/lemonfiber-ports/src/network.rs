//! Asking this machine what it calls itself.
//!
//! Behind a port because it is the one fact a test can never settle for itself:
//! every machine that runs these tests answers differently, so a test written
//! against the real one would pass where it was written and nowhere else.
//!
//! It does not fail. A machine that will not say its name and one that has no name
//! to say are the same absence to whoever asked, and an error here would make the
//! caller decide twice — once for the failure and once for the absence — about one
//! thing.
//!
//! See `.docs/architecture/ports-and-adapters.md`.

use async_trait::async_trait;

/// Asks this machine what it is called.
#[async_trait]
pub trait Site: Send + Sync {
    /// What this machine calls itself, read now rather than remembered, or nothing
    /// where it will not say.
    async fn name(&self) -> Option<String>;
}
