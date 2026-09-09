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
pub fn carryable(carrying: &[Carried], profiles: &[String]) -> Vec<Carried> {
    let refused = unmatched(carrying, profiles);
    carrying
        .iter()
        .filter(|carried| !refused.iter().any(|named| named.what == carried.name))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{carryable, missing, unmatched};
    use crate::ports::service::Carried;

    fn carried(name: &str, profile: Option<&str>) -> Carried {
        Carried {
            name: name.to_owned(),
            profile: profile.map(str::to_owned),
            folder: None,
            rest: format!("{{\"title\":\"{name}\"}}"),
        }
    }

    #[test]
    fn a_record_not_held_here_is_one_to_carry() {
        let theirs = [carried("Taskmaster", None)];
        let named: Vec<String> = missing(&theirs, &[])
            .into_iter()
            .map(|one| one.name)
            .collect();
        assert_eq!(named, vec!["Taskmaster".to_owned()]);
    }

    /// Overwriting what the operator has since changed here would be undoing their work
    /// in the name of copying it.
    #[test]
    fn a_record_already_held_here_is_left_exactly_as_it_is() {
        let theirs = [carried("Taskmaster", None)];
        let ours = [carried("Taskmaster", Some("Any"))];
        assert!(missing(&theirs, &ours).is_empty());
    }

    #[test]
    fn a_profile_this_stack_has_is_no_obstacle() {
        let carrying = [carried("Taskmaster", Some("HD"))];
        assert!(unmatched(&carrying, &["HD".to_owned()]).is_empty());
        assert_eq!(carryable(&carrying, &["HD".to_owned()]).len(), 1);
    }

    /// The one case worth stopping for: carried without its profile it would be
    /// re-graded, which is worse than not being carried.
    #[test]
    fn a_profile_this_stack_lacks_stops_the_record_and_says_why() {
        let carrying = [carried("Taskmaster", Some("Bespoke"))];
        let refused = unmatched(&carrying, &["HD".to_owned()]);
        let said = refused
            .first()
            .map(|one| one.because.clone())
            .unwrap_or_default();
        assert!(said.contains("Bespoke"), "{said}");
        assert!(said.contains("re-graded"), "{said}");
        assert!(carryable(&carrying, &["HD".to_owned()]).is_empty());
    }

    /// An indexer follows no profile at all, and nothing about it needs one.
    #[test]
    fn a_record_following_no_profile_is_carried_freely() {
        let carrying = [carried("an indexer", None)];
        assert!(unmatched(&carrying, &[]).is_empty());
        assert_eq!(carryable(&carrying, &[]).len(), 1);
    }

    /// One bad record does not stop the rest.
    #[test]
    fn the_records_that_can_be_carried_are_carried() {
        let carrying = [
            carried("Taskmaster", Some("HD")),
            carried("Bake Off", Some("Bespoke")),
        ];
        let going: Vec<String> = carryable(&carrying, &["HD".to_owned()])
            .into_iter()
            .map(|one| one.name)
            .collect();
        assert_eq!(going, vec!["Taskmaster".to_owned()]);
    }
}
