//! Saying what a wait is waiting for, while it is still waiting.
//!
//! A command that reaches the container engine spends minutes doing it, and for most
//! of that there is nothing to run and nothing to read — only a poll that keeps
//! coming back with the same answer. Silence there is indistinguishable from a hang.
//!
//! The logic above this boundary has no terminal and must not gain one, so it says
//! what it is waiting for and a surface decides where those words land: an indented
//! line under the command, an event on the stream a browser holds open. That is the
//! same division a streamed pull already makes, arriving as a port rather than as a
//! channel because a wait is reached through a dispatcher that hands back one value
//! at the end.

use async_trait::async_trait;

/// Where a long-running command says what it is doing while it does it.
#[async_trait]
pub trait Narrator: Send + Sync {
    /// Say one line, now.
    ///
    /// Nothing comes back, and nothing can go wrong that a caller could act on: a
    /// wait that could fail because nobody was listening would be a wait made worse
    /// by the reporting added to it.
    async fn say(&self, said: &str);
}

/// The narrator for a run nobody is listening to.
///
/// A value rather than an absence, so a wait says what it is waiting for exactly
/// once in the source — the alternative is every call site asking whether anyone is
/// there, which is the branch that gets forgotten on the path that matters.
pub struct Silent;

#[async_trait]
impl Narrator for Silent {
    async fn say(&self, _said: &str) {}
}
