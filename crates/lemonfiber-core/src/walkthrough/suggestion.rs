//! What to suggest to someone with an empty library and no idea what to type.
//!
//! A first attempt that fails because the operator picked something obscure teaches
//! exactly the wrong lesson: they conclude the stack is broken when what happened is that
//! nobody is carrying a 1997 Latvian documentary. So the walkthrough can suggest
//! something instead of asking a newcomer to guess.
//!
//! What makes a suggestion safe is that it is widely carried, so the search returns
//! something whatever indexers the operator happens to have. The freely-licensed titles
//! are safest of all — they are mirrored everywhere, by everyone, deliberately — and they
//! are listed first for that reason. Nothing here is a recommendation about what to
//! watch; it is a list of things likely to prove the pipeline works.

use serde::{Deserialize, Serialize};

use crate::recyclarr::Kind;

/// How confident the walkthrough is that a search for this will return something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Availability {
    /// Published under a licence that permits anyone to distribute it, and mirrored
    /// widely because of it. The safest possible first attempt.
    FreelyLicensed,
    /// Widely carried, so most indexers will have it.
    WidelyCarried,
}

impl Availability {
    /// Why this is a safe thing to try first.
    #[must_use]
    pub const fn because(self) -> &'static str {
        match self {
            Self::FreelyLicensed => "freely licensed, so it is mirrored everywhere",
            Self::WidelyCarried => "widely carried, so most indexers will have it",
        }
    }
}

/// Something to try first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Suggestion {
    /// What to search for.
    pub title: &'static str,
    /// The kind of service that would handle it.
    pub kind: Kind,
    /// Why it is a safe first attempt.
    pub availability: Availability,
}

/// The suggestions, freely-licensed first, so the safest is the default.
///
/// Short on purpose. A long list is a curation problem that ages badly; what is needed is
/// two or three things likely to work, and a way to type something else.
pub const SUGGESTIONS: &[Suggestion] = &[
    Suggestion {
        title: "Big Buck Bunny",
        kind: Kind::Radarr,
        availability: Availability::FreelyLicensed,
    },
    Suggestion {
        title: "Sintel",
        kind: Kind::Radarr,
        availability: Availability::FreelyLicensed,
    },
    Suggestion {
        title: "Tears of Steel",
        kind: Kind::Radarr,
        availability: Availability::FreelyLicensed,
    },
    Suggestion {
        title: "Pioneer One",
        kind: Kind::Sonarr,
        availability: Availability::FreelyLicensed,
    },
];

impl Suggestion {
    /// The suggestions a service of this kind could act on.
    #[must_use]
    pub fn for_kind(kind: Kind) -> Vec<Self> {
        SUGGESTIONS
            .iter()
            .filter(|suggestion| suggestion.kind == kind)
            .copied()
            .collect()
    }

    /// Everything the running stack could handle, safest first.
    #[must_use]
    pub fn for_kinds(kinds: &[Kind]) -> Vec<Self> {
        SUGGESTIONS
            .iter()
            .filter(|suggestion| kinds.contains(&suggestion.kind))
            .copied()
            .collect()
    }

    /// The safest thing to try, of everything the running stack could handle.
    ///
    /// `kinds` is what is actually running: a stack with no film service should not be
    /// offered a film, however safe the film would have been.
    #[must_use]
    pub fn safest(kinds: &[Kind]) -> Option<Self> {
        Self::for_kinds(kinds).first().copied()
    }

    /// Something safe for a service of this kind to be asked for.
    ///
    /// A value rather than a maybe: every kind lemonfiber knows has something worth
    /// trying, so a caller holding a running service never has to handle "and if there
    /// were nothing" — a branch it could not reach and could not test.
    #[must_use]
    pub const fn safe_for(kind: Kind) -> &'static str {
        match kind {
            Kind::Sonarr => "Pioneer One",
            Kind::Radarr => "Big Buck Bunny",
        }
    }

    /// The suggestion said in one line, as it is put to the operator.
    #[must_use]
    pub fn said(&self) -> String {
        format!("{} — {}", self.title, self.availability.because())
    }
}

#[cfg(test)]
mod tests {
    use super::{Availability, Suggestion, SUGGESTIONS};
    use crate::recyclarr::Kind;

    /// Safest first, which is the order the list is held in.
    const ORDER: [Availability; 2] = [Availability::FreelyLicensed, Availability::WidelyCarried];

    #[test]
    fn the_safest_suggestions_come_first() {
        // Freely-licensed titles are mirrored deliberately and by everyone, so they are
        // the likeliest first attempt to succeed. Ordering is the whole guarantee here.
        let ranks: Vec<usize> = SUGGESTIONS
            .iter()
            .map(|suggestion| {
                ORDER
                    .iter()
                    .position(|a| *a == suggestion.availability)
                    .unwrap_or(0)
            })
            .collect();
        let mut sorted = ranks.clone();
        sorted.sort_unstable();
        assert_eq!(ranks, sorted, "freely-licensed titles come first");
    }

    #[test]
    fn a_stack_is_only_suggested_what_it_can_handle() {
        // A stack with no film service should not be offered a film, however safe.
        let television = Suggestion::safest(&[Kind::Sonarr]);
        assert_eq!(television.map(|s| s.kind), Some(Kind::Sonarr));
        let film = Suggestion::safest(&[Kind::Radarr]);
        assert_eq!(film.map(|s| s.kind), Some(Kind::Radarr));
        assert_eq!(
            Suggestion::safest(&[]),
            None,
            "nothing running, nothing to suggest"
        );
        assert!(Suggestion::for_kinds(&[]).is_empty());
        assert_eq!(
            Suggestion::for_kinds(&[Kind::Sonarr, Kind::Radarr]).len(),
            SUGGESTIONS.len(),
            "a stack running both is offered everything"
        );
    }

    #[test]
    fn both_kinds_have_something_to_suggest() {
        // A stack running only one of the two is still walked, so each kind needs at
        // least one thing to try.
        for kind in [Kind::Sonarr, Kind::Radarr] {
            assert!(
                !Suggestion::for_kind(kind).is_empty(),
                "nothing to suggest for {kind:?}"
            );
        }
    }

    #[test]
    fn every_kind_has_something_safe_to_be_asked_for_without_a_maybe() {
        // A caller holding a running service should never have to handle "and if there
        // were nothing to suggest" — a branch it could not reach and could not test.
        for kind in [Kind::Sonarr, Kind::Radarr] {
            let safe = Suggestion::safe_for(kind);
            assert!(
                Suggestion::for_kind(kind).iter().any(|s| s.title == safe),
                "{safe} is not among the suggestions for {kind:?}"
            );
        }
    }

    #[test]
    fn a_suggestion_says_what_it_is_and_why_it_is_safe() {
        let said = SUGGESTIONS
            .first()
            .map(|suggestion| (suggestion.said(), suggestion.title, suggestion.availability));
        assert!(
            said.is_some_and(|(said, title, availability)| said.starts_with(title)
                && said.contains(availability.because())),
            "a suggestion names itself and says why it is safe"
        );
    }

    #[test]
    fn every_reason_a_suggestion_is_safe_reads_as_a_reason() {
        for availability in [Availability::FreelyLicensed, Availability::WidelyCarried] {
            assert!(availability.because().contains("so"), "{availability:?}");
        }
    }
}
