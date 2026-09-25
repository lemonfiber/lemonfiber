//! Which of an operator's own records would be carried across, and which would not.
//!
//! Matching is by name, because that is the only thing two stacks agree on. Everything
//! numbered — a record's id, the profile it follows, the folder it sits under — belongs
//! to the stack that numbered it, and is resolved again at the far end.
//!
//! Nothing here can reach a service. It is given what both sides hold and returns what
//! that amounts to, so every arrangement an import has to get right can be exercised
//! without two stacks running.

use crate::model::UnsupportedReport;
use crate::ports::service::Carried;

/// The records held there that are not held here.
///
/// A record already here is left exactly as it is. An import that overwrote what an
/// operator has since changed on the new stack would be undoing their work in the name
/// of copying it.
#[must_use]
pub fn missing(theirs: &[Carried], ours: &[Carried]) -> Vec<Carried> {
    theirs
        .iter()
        .filter(|carried| !ours.iter().any(|held| held.name == carried.name))
        .cloned()
        .collect()
}

/// The records that cannot be carried, and why.
///
/// A record following a quality profile this stack does not have is the one case worth
/// stopping for: carried without it, it would be given whichever profile the service
/// defaults to, and an operator would find their library quietly re-graded rather than
/// copied.
#[must_use]
pub fn unmatched(carrying: &[Carried], profiles: &[String]) -> Vec<UnsupportedReport> {
    carrying
        .iter()
        .filter_map(|carried| {
            let wanted = carried.profile.as_ref()?;
            if profiles.iter().any(|held| held == wanted) {
                return None;
            }
            Some(UnsupportedReport {
                what: carried.name.clone(),
                because: format!(
                    "it follows a quality profile called {wanted}, which this stack does \
                     not have — carried without it, it would be re-graded rather than copied"
                ),
            })
        })
        .collect()
}

/// The records that can be carried, which is what is left once the rest are named.
#[must_use]
pub(crate) fn carryable(carrying: &[Carried], profiles: &[String]) -> Vec<Carried> {
    let refused = unmatched(carrying, profiles);
    carrying
        .iter()
        .filter(|carried| !refused.iter().any(|named| named.what == carried.name))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests;
