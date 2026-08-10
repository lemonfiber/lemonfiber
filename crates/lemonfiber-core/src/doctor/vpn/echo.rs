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
mod tests {
    use super::Seen;

    /// What a source answered — wrapped, since a source that could not be reached
    /// answers `None` and that distinction is what half these tests are about.
    fn said(address: &str) -> Option<String> {
        (!address.is_empty()).then(|| address.to_owned())
    }

    #[test]
    fn sources_that_agree_settle_the_address() {
        let seen = Seen::of(&[said("203.0.113.7"), said("203.0.113.7")]);
        assert_eq!(seen, Seen::Agreed("203.0.113.7".to_owned()));
        assert_eq!(seen.settled(), Some("203.0.113.7"));
        assert_eq!(seen.said(), None, "nothing to report");
    }

    #[test]
    fn sources_that_disagree_settle_nothing_and_say_so() {
        // Picking a winner would be inventing the answer: there is no basis to
        // prefer one stranger's account over another's, and every verdict
        // downstream would inherit the invention without knowing.
        let seen = Seen::of(&[said("203.0.113.7"), said("198.51.100.9")]);
        assert_eq!(seen.settled(), None);
        let reported = seen.said().unwrap_or_default();
        assert!(reported.contains("203.0.113.7"), "{reported}");
        assert!(reported.contains("198.51.100.9"), "{reported}");
    }

    #[test]
    fn a_source_that_could_not_be_reached_is_silence_rather_than_a_contradiction() {
        // Otherwise a conflict would be reported every time an echo went down,
        // which is often and means nothing.
        let seen = Seen::of(&[said("203.0.113.7"), None]);
        assert_eq!(seen, Seen::Agreed("203.0.113.7".to_owned()));
        assert_eq!(seen.settled(), Some("203.0.113.7"));
    }

    #[test]
    fn nothing_answering_at_all_is_its_own_state() {
        // Distinct from agreement on purpose: no address is not the same claim as
        // an address everybody confirmed, and the verdict differs.
        let seen = Seen::of(&[None, None]);
        assert_eq!(seen, Seen::Silent);
        assert_eq!(seen.settled(), None);
        assert_eq!(seen.said(), None, "silence is not a disagreement to report");
        assert_eq!(Seen::of(&[]), Seen::Silent);
    }

    #[test]
    fn one_source_answering_alone_is_taken_at_its_word() {
        // Better than nothing, and the operator configured only one. The check is
        // then as trustworthy as that source, which is why more than one is asked.
        assert_eq!(
            Seen::of(&[said("203.0.113.7")]).settled(),
            Some("203.0.113.7")
        );
    }

    #[test]
    fn three_sources_with_one_dissenter_is_still_a_disagreement() {
        // Not a vote. Two against one is not evidence about which is right — a
        // majority of strangers is still strangers, and the one dissenting may be
        // the only one not behind a cache.
        let seen = Seen::of(&[
            said("203.0.113.7"),
            said("203.0.113.7"),
            said("198.51.100.9"),
        ]);
        assert_eq!(seen.settled(), None);
        assert!(matches!(seen, Seen::Disagreed(ref heard) if heard.len() == 2));
    }
}
