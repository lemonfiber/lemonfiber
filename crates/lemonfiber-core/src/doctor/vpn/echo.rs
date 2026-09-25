//! Asking more than one source what our address looks like from outside.
//!
//! The whole leak check rests on one number: the address the world sees. Every
//! verdict downstream — behind the tunnel, leaking, unverified — is a comparison
//! against it. Ask one service and the check is exactly as trustworthy as that
//! service, which is a strange place to have put the entire guarantee: an echo
//! that is misconfigured, cached behind a proxy, or simply lying returns a
//! plausible address and the check reports **pass** while traffic leaves in the
//! clear.
//!
//! So more than one is asked, and disagreement is reported rather than resolved.
//! Picking a winner would be inventing the answer: there is no basis on which to
//! prefer one stranger's account over another's, and a check that quietly chose
//! would be at its least trustworthy exactly when it mattered most.
//!
//! Silence is not disagreement. A source that could not be reached has said
//! nothing, and one source answering while another is down is an ordinary
//! internet rather than a contradiction.

use serde::{Deserialize, Serialize};

/// What the sources between them said the address is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Seen {
    /// Every source that answered gave the same address.
    Agreed(String),
    /// The sources that answered did not agree, listed as they answered.
    Disagreed(Vec<String>),
    /// Nothing answered at all.
    Silent,
}

impl Seen {
    /// What a set of answers amounts to.
    ///
    /// `None` from a source is one that could not be reached, which is silence
    /// rather than a contradiction — a check that treated an unreachable source as
    /// disagreement would report a conflict every time an echo went down.
    #[must_use]
    pub fn of(answers: &[Option<String>]) -> Self {
        let mut heard: Vec<String> = answers.iter().flatten().cloned().collect();
        heard.dedup_by(|left, right| left == right);
        heard.sort_unstable();
        heard.dedup();
        match heard.len() {
            0 => Self::Silent,
            1 => heard.into_iter().next().map_or(Self::Silent, Self::Agreed),
            _ => Self::Disagreed(heard),
        }
    }

    /// The address, where there is one anybody can rely on.
    ///
    /// Nothing where the sources disagreed: an answer chosen from among
    /// contradictory ones is an invention, and every verdict downstream would
    /// inherit it without knowing.
    #[must_use]
    pub fn settled(&self) -> Option<&str> {
        match self {
            Self::Agreed(address) => Some(address),
            Self::Disagreed(_) | Self::Silent => None,
        }
    }

    /// How a disagreement reads, where there is one.
    #[must_use]
    pub fn said(&self) -> Option<String> {
        match self {
            Self::Disagreed(heard) => Some(format!(
                "the address services disagree about what this machine looks like from \
                 outside: {}",
                heard.join(", ")
            )),
            Self::Agreed(_) | Self::Silent => None,
        }
    }
}

#[cfg(test)]
mod tests;
