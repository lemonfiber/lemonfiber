//! What a member asks for when they sit down to watch.

use super::{Asker, Whom};
use crate::model::WITHHELD;
use crate::ports::service::HowFar;

/// What a member asks of the media server through the core: one title, what they were
/// part-way through, a grant for a device to play on, how far they got, the live TV
/// channels they may watch, a channel to watch and a title to play in a browser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Viewing {
    /// What one title is, as the member may see it.
    Title {
        /// Whose shelf it is read from.
        member: Whom,
        /// The title, by the identifier the shelf lists it under.
        id: String,
        /// Who asked.
        asker: Asker,
    },
    /// What a member was part-way through, most recent first.
    PartWay {
        /// Whose it is.
        member: Whom,
        /// How many to answer with.
        most: u32,
        /// Who asked.
        asker: Asker,
    },
    /// Open a session on a member's own account for one of their devices.
    Grant {
        /// The member the device plays as, by name or id.
        member: String,
        /// The id the player keeps for the device it plays on.
        device: String,
    },
    /// Record how far through one title a member is.
    Watched {
        /// The member, by name or id.
        member: String,
        /// The title or episode, by the identifier the shelf lists it under.
        id: String,
        /// How far in, and whether they finished it.
        how_far: HowFar,
    },
    /// The live TV channels a member may watch, each with the programme on it now.
    Channels {
        /// The member, by name or id.
        member: String,
    },
    /// Open a channel's live stream for a member, renew its passes, or close it.
    Tune {
        /// The member, by name or id.
        member: String,
        /// What is asked of the live stream.
        live: Live,
        /// Who asked.
        asker: Asker,
    },
    /// Where one title plays from in a member's browser, on a pass holding a session on
    /// their own account.
    Play {
        /// The member, by name or id.
        member: String,
        /// The title or episode, by the identifier the shelf lists it under.
        id: String,
        /// What the pass is made from.
        from: PlayFrom,
        /// Who asked.
        asker: Asker,
    },
}

/// What is asked of a channel's live stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Live {
    /// Open this channel's live stream.
    Open(String),
    /// New passes for the live stream named, which the core opened for the member.
    Renew(String),
    /// Close the live stream named.
    Stop(String),
}

/// What a browser's pass is made from.
#[derive(Clone, PartialEq, Eq)]
pub enum PlayFrom {
    /// A new session on the member's account for the browser by this device id.
    Device(String),
    /// The session the pass a browser holds seals.
    Pass(String),
}

/// Written by hand so a pass is never in what is debugged or logged.
impl std::fmt::Debug for PlayFrom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Device(device) => f.debug_tuple("Device").field(device).finish(),
            Self::Pass(_) => f.debug_tuple("Pass").field(&WITHHELD).finish(),
        }
    }
}
