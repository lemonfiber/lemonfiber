//! What this build offers a plugin, which is the set `[requires]` is answered from.
//!
//! The third of the three things this system calls capabilities, and the one that runs
//! the other way: [`crate::vocabulary`] is what a *service* can do, `grants` in the
//! stack manifest is what the kernel lets a container do, and this is what lemonfiber
//! offers the manifest reading it. No name appears in two of them, because a name whose
//! meaning depended on the field it sat in is exactly the confusion the kernel set was
//! renamed out of.
//!
//! **A plugin names what it needs, and is answered by name.** A version number cannot
//! answer the question, because it conflates *older* with *missing something you
//! needed*: a plugin written a year ago against a stack it has never met keeps working
//! for as long as the things it actually uses still exist, and when it stops working the
//! message has to say which thing went.
//!
//! **Derived, not listed.** Every published extension point names the capability a
//! contribution there asks for, and publishing a point *is* the offer — so the points
//! are read for it rather than their names being written out again here. A second copy
//! would be free to go on offering something a withdrawn point no longer takes, and a
//! plugin would be told it could have what nothing would give it.
//!
//! What is written out is the set of offers that are nobody's point: each arrives with
//! the mechanism that provides it, and until that mechanism is here the honest answer to
//! a manifest asking for one is a refusal naming it.

use std::collections::BTreeSet;

use crate::extension;

/// The generation this set is at.
///
/// Monotonic. Advanced by a withdrawal or by an offer narrowing; adding one does not
/// move it, because nothing already written stops being true.
pub const OFFERED_VERSION: u32 = 1;

/// An offer this build makes that no published extension point makes for it.
///
/// Empty while every mechanism a plugin can ask for is either a point's or unbuilt.
/// An entry here is a promise that something in this build does the thing, so one is
/// added by the change that makes it true rather than by the change that wants it.
const BESIDE_THE_POINTS: &[&str] = &[];

/// Every capability this build offers a plugin, in one stable order.
#[must_use]
pub fn offered() -> BTreeSet<&'static str> {
    extension::POINTS
        .iter()
        .map(|point| point.requires)
        .chain(BESIDE_THE_POINTS.iter().copied())
        .collect()
}

/// Whether this build offers a capability a manifest asks for by this name.
#[must_use]
pub fn offers(name: &str) -> bool {
    offered().contains(name)
}

#[cfg(test)]
mod tests {
    use super::{offered, offers, BESIDE_THE_POINTS};
    use crate::extension;

    /// What a point asks for is what this build offers by publishing the point.
    #[test]
    fn every_published_point_is_an_offer() {
        for point in extension::POINTS {
            assert!(
                offers(point.requires),
                "{} asks for {}, which nothing offers",
                point.name,
                point.requires
            );
        }
        assert!(
            !extension::POINTS.is_empty(),
            "a build publishing no point at all would make that sweep vacuous"
        );
    }

    /// A capability nothing here provides is not offered, whatever a manifest asks.
    #[test]
    fn a_capability_no_mechanism_here_provides_is_not_offered() {
        assert!(!offers("service.add"));
        assert!(!offers("recipe.run"));
    }

    /// The set is derived, so removing what it is derived from empties it.
    ///
    /// The property worth holding is that this cannot silently answer yes to
    /// everything: with nothing to derive from and nothing written beside it, the
    /// answer to every name is no.
    #[test]
    fn with_no_point_and_nothing_beside_them_nothing_is_offered() {
        assert_eq!(
            BESIDE_THE_POINTS.len() + extension::POINTS.len(),
            offered().len() + repeated(),
            "the offered set is exactly what it is derived from"
        );
    }

    /// How many points share a capability with another point or with a listed offer.
    fn repeated() -> usize {
        let derived: Vec<&str> = extension::POINTS
            .iter()
            .map(|point| point.requires)
            .chain(BESIDE_THE_POINTS.iter().copied())
            .collect();
        derived.len() - offered().len()
    }
}
