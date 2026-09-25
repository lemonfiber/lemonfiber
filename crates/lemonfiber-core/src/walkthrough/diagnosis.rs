//! Why a walkthrough stopped, said at the step it stopped at.
//!
//! Failure is the useful case. A stack whose import is broken looks, from outside,
//! exactly like a stack nobody has asked anything of — and the operator finds out three
//! days later, as a mysterious absence, with no idea which of six services to blame. The
//! walkthrough breaks that on purpose, at the one moment they are engaged, expecting to
//! interact, and willing to fix things.
//!
//! So every stop names its step, says what happened in the operator's language, and
//! carries a remedy. The distinctions that matter are the ones that look identical and
//! are not: indexers that failed against indexers that worked and found nothing, a
//! download that is slow against one that has stopped, an import that could not run
//! against one that ran and copied.

use serde::{Deserialize, Serialize};

use super::Step;

/// Why a walkthrough could not go on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Reason {
    /// No indexer is configured, so there is nothing to search. Not a failure of the
    /// walk — a reason not to offer it at all.
    NoIndexers,
    /// The search could not be run: the indexers, or the service holding them, would not
    /// answer.
    IndexersFailed,
    /// The search ran cleanly and matched nothing. An entirely different problem from
    /// the one above, and the one operators most often mistake for it.
    NothingMatched,
    /// Releases exist, but none meets the chosen quality preset. The stack is working;
    /// the preset and what is out there disagree.
    NoneMetThePreset,
    /// Torrents are in play and the tunnel is not verified up. Nothing is grabbed.
    TunnelDown,
    /// The release was never handed to a download client.
    NotGrabbed,
    /// The download stopped making progress.
    Stalled,
    /// The download finished and the library manager would not take it.
    ImportFailed,
    /// There is no media server in the running form to make it playable.
    NoMediaServer,
    /// It was imported, the media server was told, and it still cannot be found there.
    NotVisible,
}

impl Reason {
    /// The step this stops the walk at, so the operator is told where in a chain of six
    /// services the chain broke.
    #[must_use]
    pub const fn step(self) -> Step {
        match self {
            Self::NoIndexers => Step::Choosing,
            Self::IndexersFailed | Self::NothingMatched | Self::NoneMetThePreset => Step::Searching,
            Self::TunnelDown | Self::NotGrabbed => Step::Grabbing,
            Self::Stalled => Step::Downloading,
            Self::ImportFailed => Step::Importing,
            Self::NoMediaServer | Self::NotVisible => Step::Scanning,
        }
    }

    /// What happened, in the operator's words rather than the service's.
    #[must_use]
    pub const fn said(self) -> &'static str {
        match self {
            Self::NoIndexers => "There are no indexers configured, so there is nothing to search",
            Self::IndexersFailed => "The indexers could not be searched",
            Self::NothingMatched => "The indexers answered, and none of them had this",
            Self::NoneMetThePreset => {
                "Releases exist, but none of them meets the quality you asked for"
            }
            Self::TunnelDown => {
                "This would be a torrent, and the VPN is not confirmed up — nothing was grabbed"
            }
            Self::NotGrabbed => "Nothing was handed to a download client",
            Self::Stalled => "The download stopped making progress",
            Self::ImportFailed => {
                "The download finished, and it could not be filed into the library"
            }
            Self::NoMediaServer => {
                "It is on disk, and there is no media server running to play it from"
            }
            Self::NotVisible => "It was filed, the library was told to look, and it is not there",
        }
    }

    /// What to do about it. One action, the one most likely to be the whole fix.
    #[must_use]
    pub const fn remedy(self) -> &'static str {
        match self {
            Self::NoIndexers => "Add an indexer in Prowlarr, then run `lemonfiber walkthrough`",
            Self::IndexersFailed => "Run `lemonfiber doctor` — it tests each indexer and says which",
            Self::NothingMatched => "Try something else, or check the indexer covers this category",
            Self::NoneMetThePreset => {
                "Run `lemonfiber quality` to see the preset, or choose something more widely available"
            }
            Self::TunnelDown => "Run `lemonfiber doctor --only vpn` and bring the tunnel up",
            Self::NotGrabbed => "Run `lemonfiber doctor` — the download client is usually the reason",
            Self::Stalled => "Run `lemonfiber stuck` to see it alongside anything else that is stalled",
            Self::ImportFailed => {
                "Check that the downloads and the library are one volume at one path in both containers"
            }
            Self::NoMediaServer => "Start a form that includes the media server, such as `lemonfiber up tv`",
            Self::NotVisible => "Give the library a moment and look again, or rescan it from its own interface",
        }
    }

    /// Whether this is a fault in the stack, as against an answer about the world.
    ///
    /// Indexers that answered and had nothing is not a broken stack, and telling the
    /// operator to go fixing things would send them after a fault that is not there.
    #[must_use]
    pub const fn is_a_fault(self) -> bool {
        !matches!(self, Self::NothingMatched | Self::NoneMetThePreset)
    }

    /// Every reason, so a caller can prove it handles all of them.
    #[must_use]
    pub const fn all() -> [Self; 10] {
        [
            Self::NoIndexers,
            Self::IndexersFailed,
            Self::NothingMatched,
            Self::NoneMetThePreset,
            Self::TunnelDown,
            Self::NotGrabbed,
            Self::Stalled,
            Self::ImportFailed,
            Self::NoMediaServer,
            Self::NotVisible,
        ]
    }
}

/// A walkthrough that stopped: where, why, what the services were saying, and what to do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Stopped {
    /// The step it stopped at.
    pub step: Step,
    /// Why.
    pub reason: Reason,
    /// What the services involved were saying at the time, shown inline rather than left
    /// for the operator to go and find — a fault report they have to research is a fault
    /// report they abandon.
    pub logs: Vec<String>,
    /// The one thing to try.
    pub remedy: String,
}

impl Stopped {
    /// A stop with nothing to quote — the service said nothing, or the reason is about
    /// configuration rather than something that ran.
    #[must_use]
    pub fn plain(reason: Reason) -> Self {
        Self {
            step: reason.step(),
            reason,
            logs: Vec::new(),
            remedy: reason.remedy().to_owned(),
        }
    }

    /// A stop with the services' own words attached.
    #[must_use]
    pub fn quoting(reason: Reason, logs: Vec<String>) -> Self {
        Self {
            logs,
            ..Self::plain(reason)
        }
    }
}

#[cfg(test)]
mod tests;
