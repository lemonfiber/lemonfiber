//! The capabilities of the curators a household member asks music and books of, each
//! filled by the curator that files that kind of media.

use super::super::{Capability, Constraint, Credential, Probe, Requirement};

/// What a member asks for of music.
pub(super) const MUSIC_CURATE: Capability = Capability {
    name: "music.curate",
    summary: "Looks albums and artists up by words, and takes a member's ask for one.",
    contract: "Answers a lookup of albums or artists by words with what the catalogue holds, \
               takes an ask for one album or artist into its wanted list at the profile it is \
               given, and says the units it answers for. Nothing it holds is readable without \
               the operator's credential.",
    probes: CURATED,
    listens: true,
};

/// What a member asks for of books.
pub(super) const BOOKS_CURATE: Capability = Capability {
    name: "books.curate",
    summary: "Looks books and authors up by words, and takes a member's ask for one.",
    contract: "Answers a lookup of books or authors by words with what the catalogue holds, \
               takes an ask for one book or author into its wanted list at the profile it is \
               given, and says the units it answers for. Nothing it holds is readable without \
               the operator's credential.",
    probes: CURATED,
    listens: true,
};

/// What a curator a member asks of shows: that it refuses the anonymous, and answers a
/// lookup to the operator.
const CURATED: &[Probe] = &[
    Probe {
        id: "guarded",
        title: "An anonymous caller is refused a lookup",
        asks: "a lookup by words, presenting nothing",
        why: "A curator holds every key the stack was given and can be made to download \
              anything. Its API answering the network at all is the failure; whatever the \
              lookup would have found is beside the point.",
        credential: Credential::None,
        requires: Requirement::Request {
            status: &[401, 403],
            body: &[],
        },
    },
    Probe {
        id: "looked-up",
        title: "It answers a lookup with what the catalogue holds",
        asks: "the same lookup, with the credential the operator holds",
        why: "A lookup is what a member's ask starts from, and the half of this that can be \
              demonstrated without asking for anything.",
        credential: Credential::Operator,
        requires: Requirement::Request {
            status: &[200],
            body: &[Constraint::Json, Constraint::JsonArrayMin],
        },
    },
];
