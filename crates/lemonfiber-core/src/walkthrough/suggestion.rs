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
    /// Everything the running stack could handle, safest first.
    #[must_use]
    pub(crate) fn for_kinds(kinds: &[Kind]) -> Vec<Self> {
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
    pub(crate) const fn safe_for(kind: Kind) -> &'static str {
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
mod tests;
