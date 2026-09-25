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
mod tests;
