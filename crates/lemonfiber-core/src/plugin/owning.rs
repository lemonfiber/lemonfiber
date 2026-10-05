//! Whose a journalled change is, where the answer is a plugin.
//!
//! Removing or updating a plugin puts back every change that is the plugin's, so the
//! question decides what leaves the machine with it. It is answered from what the
//! entry itself carries — the operation it was journalled under, and for a record
//! written under the plugin's bare id, the kind of write — and from nothing a plugin's
//! author chose beyond its id.

use crate::journal::{Change, Kind};

/// Whose a plugin's changes are: the owner its regions' markers name, and the
/// operation every change it makes is journalled under.
///
/// Not the bare id. An id is a stranger's choice from the same letters lemonfiber's own
/// operations are named in, and removing a plugin puts back every change journalled
/// under its operation — so a plugin called after one of them would take that
/// operation's changes back with it. The word in front is one no operation of
/// lemonfiber's own begins with.
#[must_use]
pub fn owner(plugin: &str) -> String {
    format!("plugin {plugin}")
}

/// Whether `change` is one of the plugin's own.
///
/// Every change journalled under [`owner`] is. A journal on disk can also hold a
/// plugin's writes under its bare id, and such an entry is the plugin's only where all
/// three hold: the id is none of [`crate::journal::OPERATIONS`], the entry is a path
/// made or a region written — the only two writes an install journals — and a region's
/// markers name the plugin as their owner. Each narrows what an operation of
/// lemonfiber's own could share with a plugin's id, so a plugin called after one takes
/// none of its changes, and the two kinds still leave no plugin write unclaimed.
#[must_use]
pub fn owns(plugin: &str, change: &Change) -> bool {
    if change.operation == owner(plugin) {
        return true;
    }
    if change.operation != plugin || crate::journal::OPERATIONS.contains(&plugin) {
        return false;
    }
    match &change.kind {
        Kind::Made { .. } => true,
        Kind::Region { owner: marked, .. } => *marked == owner(plugin),
        _ => false,
    }
}

#[cfg(test)]
mod tests;
