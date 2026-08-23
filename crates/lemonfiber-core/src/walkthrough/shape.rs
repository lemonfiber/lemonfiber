//! Which walkthrough a stack deserves, and whether to offer one at all.
//!
//! Not every stack can fetch something. One configured for neither usenet nor torrents is
//! a media server over media the household already owns, and offering to acquire
//! something would be the product misunderstanding its own operator. That stack still has
//! a first-content question worth answering — *can Jellyfin see my files?* — so it gets a
//! walkthrough of its own rather than none.
//!
//! And a stack with no indexer configured cannot search. Offering a walk that must stop
//! at the first step teaches the operator that the product does not know what it is doing;
//! pointing at the missing prerequisite teaches them what to do next.

use serde::{Deserialize, Serialize};

use super::Reason;

/// Which walkthrough this stack is offered.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Shape {
    /// The full walk: search, grab, download, import, and see it in the library.
    #[default]
    Pipeline,
    /// The walk for a stack that acquires nothing: point at media already on disk and
    /// confirm the media server can see it.
    LibraryOnly,
}

impl Shape {
    /// What this walk sets out to prove, said before it starts so the operator knows
    /// what they are watching for.
    #[must_use]
    pub const fn proves(self) -> &'static str {
        match self {
            Self::Pipeline => {
                "that every link works — the indexers, the download client, the library \
                 manager, the disk, and the media server"
            }
            Self::LibraryOnly => {
                "that the media server can see the media you already have, and play it"
            }
        }
    }
}

/// Whether a walkthrough is worth offering to this stack, and which one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    /// Offer this shape of walk.
    Offer(Shape),
    /// Do not offer. What is missing first, said as the reason it would have stopped at.
    Not(Reason),
}

impl Why {
    /// What to offer a stack that acquires content where `indexers` says so, and one that
    /// does not where it acquires nothing.
    ///
    /// `acquires` is whether any download protocol is configured at all; `indexers` is
    /// whether anything is configured to search. A stack that acquires nothing needs no
    /// indexer, so the two are asked in that order rather than both at once.
    #[must_use]
    pub const fn of(acquires: bool, indexers: bool) -> Self {
        if !acquires {
            return Self::Offer(Shape::LibraryOnly);
        }
        if indexers {
            return Self::Offer(Shape::Pipeline);
        }
        Self::Not(Reason::NoIndexers)
    }

    /// The shape to walk, where there is one.
    #[must_use]
    pub const fn shape(self) -> Option<Shape> {
        match self {
            Self::Offer(shape) => Some(shape),
            Self::Not(_) => None,
        }
    }

    /// Whether this stack is offered a walk.
    #[must_use]
    pub const fn is_offered(self) -> bool {
        self.shape().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::{Reason, Shape, Why};

    #[test]
    fn a_stack_that_downloads_and_can_search_gets_the_whole_walk() {
        assert_eq!(Why::of(true, true), Why::Offer(Shape::Pipeline));
        assert!(Why::of(true, true).is_offered());
    }

    #[test]
    fn a_stack_that_acquires_nothing_is_asked_a_different_question() {
        // A media server over media the household already owns has a first-content
        // question — can Jellyfin see my files? — and it is not "shall I fetch one".
        assert_eq!(Why::of(false, false), Why::Offer(Shape::LibraryOnly));
        assert_eq!(
            Why::of(false, true),
            Why::Offer(Shape::LibraryOnly),
            "an indexer it has no use for does not change the question"
        );
    }

    #[test]
    fn a_stack_with_nothing_to_search_is_pointed_at_the_prerequisite() {
        // Offering a walk that must stop at the first step teaches the operator that the
        // product does not know what it is doing.
        assert_eq!(Why::of(true, false), Why::Not(Reason::NoIndexers));
        assert!(!Why::of(true, false).is_offered());
        assert_eq!(Why::of(true, false).shape(), None);
    }

    #[test]
    fn each_shape_says_what_it_sets_out_to_prove() {
        assert!(Shape::Pipeline.proves().contains("every link"));
        assert!(Shape::LibraryOnly.proves().contains("already have"));
        assert_ne!(Shape::Pipeline.proves(), Shape::LibraryOnly.proves());
    }
}
