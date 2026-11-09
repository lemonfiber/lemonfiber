//! Where the household's own records are kept: why a request was turned down, the asks
//! for music and books the core keeps, when a code was first handed over, when an
//! invitation was offered, and what the household agreed about the requests nobody rules
//! on.

use std::path::PathBuf;

use super::{Paths, ASKS, EXPIRING, HANDOFFS, INVITATIONS, REFUSALS};

impl Paths {
    /// Why a request was turned down, by the reference the request is named by. Kept
    /// with configuration because it is a record of something the operator decided and
    /// the request service has nowhere to hold — a restore that lost it would leave every
    /// refusal bare again, which is the state this record exists to get a household out
    /// of.
    #[must_use]
    pub fn refusals(&self) -> PathBuf {
        self.config.join(REFUSALS)
    }

    /// Every ask for music or books a household member made, which the core keeps
    /// because no request service holds them. Kept with configuration because it is the
    /// only record of what was asked, by whom, and what the operator decided.
    #[must_use]
    pub fn asks(&self) -> PathBuf {
        self.config.join(ASKS)
    }

    /// When each person was first handed a code to sign a device in with. Kept with
    /// configuration because the media server records no such thing, and it is what tells
    /// a code still waiting on somebody's phone from one never handed over.
    #[must_use]
    pub fn handoffs(&self) -> PathBuf {
        self.config.join(HANDOFFS)
    }

    /// When each invitation was offered, which the media server may lose.
    #[must_use]
    pub fn invitations(&self) -> PathBuf {
        self.config.join(INVITATIONS)
    }

    /// What the household agreed about the requests nobody rules on.
    #[must_use]
    pub fn expiring(&self) -> PathBuf {
        self.config.join(EXPIRING)
    }
}
