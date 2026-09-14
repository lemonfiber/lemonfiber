//! Asking a machine about its own paths, which is a question about the machine
//! rather than about anything running on it.
//!
//! Its own file because it is its own question. Everything else an engine is asked
//! here is about containers — what is running, what it wrote, what it is costing —
//! and this is about the ground underneath them, asked once, by a guard, before any
//! of the rest applies.

use std::path::Path;

use async_trait::async_trait;

use super::Failure;

/// What asking an engine about a path on its own machine came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    /// The machine has it.
    There,
    /// The machine answered, and it has not.
    Absent,
    /// The question was put and what came back means neither.
    ///
    /// Its own answer rather than either of the other two, because both of those are
    /// statements about the machine and this is a statement about the asking. A
    /// caller that folded it into `There` would proceed on a check that did not
    /// happen; one that folded it into `Absent` would refuse a machine that is
    /// perfectly ready.
    Unknown,
}

/// Asking an engine whether a path is on the machine it runs on.
///
/// A trait of its own rather than another method on [`Engine`], for the reason
/// [`Images`] is apart: one guard asks it, and every other implementation of the
/// wider trait — the test fakes among them — would gain a method it never calls.
///
/// The engine is asked rather than a shell because the engine is what resolves a
/// bind mount. A login on the same host can see a different filesystem than the
/// daemon does — rootless, a user namespace, a daemon in a virtual machine — so an
/// answer from anything but the daemon is an answer about the wrong machine. It is
/// also the only one of the two that every transport can reach.
#[async_trait]
pub trait Locations: Send + Sync {
    /// Whether `path` is on the machine this engine runs on.
    ///
    /// # Errors
    ///
    /// Returns the [`Failure`] the endpoint produced where the engine could not be
    /// reached at all, which is a different thing from an answer about the path and
    /// is reported as one.
    async fn located(&self, path: &Path) -> Result<Presence, Failure>;
}
