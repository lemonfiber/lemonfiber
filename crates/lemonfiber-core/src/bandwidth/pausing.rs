//! Holding every download client still at once, and letting them go again.
//!
//! The line is sometimes wanted now rather than in the evening: a call about to start,
//! a guest about to play something. A limit is the wrong tool for that, because it is
//! a share held for hours. A pause is the whole line handed back until somebody says
//! otherwise.
//!
//! **Every client says for itself what became of it.** The answer is each client's own
//! read-back after it was asked, never an echo of the request, so a client that took the
//! request and went on fetching is named as fetching rather than counted as paused.
//!
//! **A pause holds until it is resumed.** Nothing here records it as lemonfiber's own
//! doing, so the run that starts again what a spent cap stopped has nothing of this to
//! start: a new month, a schedule boundary or a lifted cap leaves a paused client paused.

use serde::Serialize;

use super::Pulling;

/// Which of the two was asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Pausing {
    /// Stop every download client fetching.
    Pause,
    /// Let every download client fetch again.
    Resume,
}

impl Pausing {
    /// What each client is asked to end up doing.
    #[must_use]
    pub const fn wanted(self) -> Pulling {
        match self {
            Self::Pause => Pulling::Stopped,
            Self::Resume => Pulling::Fetching,
        }
    }

    /// The word the request is spelled with, as every surface spells it.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Pause => "pause",
            Self::Resume => "resume",
        }
    }
}

/// What one download client said about being paused or resumed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "PausedClient")]
pub struct Paused {
    /// The client, by the name the stack knows it under.
    pub client: String,
    /// Whether it was fetching before it was asked, where it said.
    pub was: Option<Pulling>,
    /// What it read back after it was asked. Absent on a rehearsal, which asks nothing,
    /// and where the client could not be reached.
    pub now: Option<Pulling>,
    /// Why it could not be reached, in its own words where it gave any.
    pub unreached: Option<String>,
}

impl Paused {
    /// Whether this client ended up where the request wanted it.
    #[must_use]
    pub fn kept(&self, asked: Pausing) -> bool {
        self.now == Some(asked.wanted())
    }
}

/// What pausing or resuming every download client came to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "PausingReport")]
pub struct Pauses {
    /// Which of the two was asked for.
    pub asked: Pausing,
    /// Every download client the stack runs, in the order the stack declares them.
    pub clients: Vec<Paused>,
    /// What a resume runs into where a spent cap had stopped the clients: they are let
    /// go as asked, and the cap stops them again the next time the line is checked.
    pub caution: Option<String>,
    /// Whether this was a rehearsal: what each client is doing now, with nothing asked
    /// of any of them.
    pub rehearsed: bool,
    /// The offer this answers: every client and what it said it was doing before it was
    /// asked anything, named so that a request carrying it back acts on those clients
    /// in those states or is refused.
    pub offer: String,
}

impl Pauses {
    /// Whether every client ended up where the request wanted it.
    ///
    /// A rehearsal asks nothing, so it has nothing to fall short of.
    #[must_use]
    pub fn whole(&self) -> bool {
        self.rehearsed || self.clients.iter().all(|client| client.kept(self.asked))
    }
}

/// What a resume says where the month's cap had stopped the clients.
pub(crate) const CAP_STILL_SPENT: &str =
    "This month's cap is spent, so the clients are stopped again the next time the line \
     is checked, as the cap was set to do.";

#[cfg(test)]
mod tests;
