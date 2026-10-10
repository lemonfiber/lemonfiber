//! Picking what to walk, and noticing when the stack already has it.
//!
//! Two things happen here that decide whether the rest of the walk is worth running. The
//! first is choosing something likely to succeed: an operator with an empty library has no
//! way to know that their indexers carry cartoons and not Latvian documentaries, and a
//! first attempt that fails on an obscure choice teaches them their stack is broken.
//!
//! The second is looking before asking. Re-acquiring something the stack already holds
//! wastes the operator's bandwidth to prove a point about a file that is already on their
//! disk, and — worse — teaches them the product does not check.

use super::walk::Walk;
use crate::app::targets::OpenCurator;
use crate::ports::service::{Catalogue, CatalogueEntry};
use crate::recyclarr::Kind;
use crate::walkthrough::{Line, Reason, Step, Suggestion};

/// What the walk settled on: which service will hold it, and what to ask that service for.
///
/// The service is borrowed rather than named, so nothing downstream has to find it again
/// — a second search for something already found is a branch that cannot fail and cannot
/// be tested either.
pub(super) struct Chosen<'a> {
    /// The service that will hold it, already open.
    pub curator: &'a OpenCurator,
    /// The catalogue entry to ask for.
    pub entry: CatalogueEntry,
    /// What to call it in every line from here on.
    pub named: String,
}

impl Chosen<'_> {
    /// Which of the two media services handles it.
    pub(super) const fn kind(&self) -> Kind {
        self.curator.kind
    }

    /// What the service calls itself, for a narration that names where work is happening.
    pub(super) fn service(&self) -> String {
        self.curator.name.clone()
    }
}

/// Why there is nothing to walk.
pub(super) enum NotChosen {
    /// The walk stops, for this reason.
    Stopped(Reason),
    /// The stack already holds it, under this name.
    AlreadyHere(String),
}

/// Choose something to walk — what the operator asked for, or the safest thing this stack
/// could handle.
pub(super) async fn choose<'a>(
    walk: &mut Walk<'_>,
    curators: &'a [OpenCurator],
    term: Option<&str>,
) -> Result<Chosen<'a>, NotChosen> {
    // Every service is asked, not only the likely one: the operator typed a title, not a
    // media type, and deciding for them which service should own it is a guess this can
    // simply avoid making. Where they typed nothing, each is asked for something safe of
    // its own kind — a stack running only films should not be offered a series.
    let mut refused = false;
    let mut said = false;
    for curator in curators {
        let asked = term.map_or_else(
            || Suggestion::safe_for(curator.kind).to_owned(),
            str::to_owned,
        );
        if !said {
            walk.say(Line::saying(Step::Choosing, asked.clone()));
            said = true;
        }
        let Ok(found) = curator.service.lookup(curator.kind, &asked).await else {
            refused = true;
            continue;
        };
        if let Some(held) = found.iter().find(|entry| entry.is_already_here()) {
            return Err(NotChosen::AlreadyHere(held.named()));
        }
        let Some(entry) = found.into_iter().find(CatalogueEntry::is_addable) else {
            continue;
        };
        let named = entry.named();
        return Ok(Chosen {
            curator,
            entry,
            named,
        });
    }

    // A catalogue that would not answer and a catalogue that answered with nothing are
    // different problems, and the operator is told which they have.
    Err(NotChosen::Stopped(if refused {
        Reason::IndexersFailed
    } else {
        Reason::NothingMatched
    }))
}

#[cfg(test)]
mod tests;
