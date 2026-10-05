//! Where the fault lies in each refusal of an install, an update or a removal.
//!
//! Listed so a client can name each code and read its status before it meets it, and
//! applied where every refusal leaves the plugin verbs, so the status a refusal is
//! answered with is the one the list gives its code: the list is the one statement of
//! it rather than a second one kept in step with the places that raise them.
//!
//! A source that holds no plugin, a plugin that is not installed, a revision the
//! repository does not have and a name the catalogue does not hold are each something
//! named that is not one of the things there are. A manifest this build refuses, a
//! plugin that would collide with what is installed, a source this machine is set not
//! to ask, an update whose source holds another plugin and a yes that leaves a value
//! unapproved are each asked in a way that cannot be answered as it stands. A machine
//! that could not write, start, fetch or verify, a catalogue entry whose source no
//! longer holds what was reviewed, a catalogue serving a release older than one this
//! machine verified, and a record of the newest one that cannot be kept, are the
//! answering.
//!
//! The record that cannot be read and the offer that moved are listed with the reads
//! and with every other moved offer, so they are not listed twice. A machine setup has
//! not run on is refused with the configuration's own code, as every record refuses it,
//! and is not a plugin's to list.

use crate::error::codes::plugin::{
    ALREADY, ANOTHER_PLUGIN, ANSWERED, CATALOGUE_OFF, CATALOGUE_REPLACED, CATALOGUE_UNREACHABLE,
    CATALOGUE_UNREADABLE, NEWEST_UNKEPT, NOTHING_TO_REMOVE, NOTHING_TO_UPDATE, NOT_AS_REVIEWED,
    NOT_CATALOGUED, NOWHERE, NO_REVISION, REFUSED, SIGNATURE_UNVERIFIED, SOURCE_OFF, SPELLED_ALIKE,
    STUCK, TWO_SOURCES, UNAPPROVED, UNFETCHED, UNPROVED, UNREADABLE, UNRECORDABLE, UNWRITABLE,
};
use crate::error::{Amiss, Code, Problem};

/// Every plugin code an install, an update or a removal is refused with, apart from the
/// record that cannot be read and the offer that moved, and where the fault lies in
/// each.
pub const REFUSALS: [(Code, Amiss); 26] = [
    (UNREADABLE, Amiss::Naming),
    (REFUSED, Amiss::Asking),
    (ALREADY, Amiss::Asking),
    (NOWHERE, Amiss::Answering),
    (UNWRITABLE, Amiss::Answering),
    (UNRECORDABLE, Amiss::Answering),
    (UNPROVED, Amiss::Answering),
    (NOTHING_TO_REMOVE, Amiss::Naming),
    (NOTHING_TO_UPDATE, Amiss::Naming),
    (STUCK, Amiss::Answering),
    (ANSWERED, Amiss::Asking),
    (TWO_SOURCES, Amiss::Asking),
    (SOURCE_OFF, Amiss::Asking),
    (UNFETCHED, Amiss::Answering),
    (NO_REVISION, Amiss::Naming),
    (CATALOGUE_OFF, Amiss::Asking),
    (CATALOGUE_UNREACHABLE, Amiss::Answering),
    (SIGNATURE_UNVERIFIED, Amiss::Answering),
    (CATALOGUE_UNREADABLE, Amiss::Answering),
    (NOT_CATALOGUED, Amiss::Naming),
    (NOT_AS_REVIEWED, Amiss::Answering),
    (SPELLED_ALIKE, Amiss::Asking),
    (UNAPPROVED, Amiss::Asking),
    (ANOTHER_PLUGIN, Amiss::Asking),
    (CATALOGUE_REPLACED, Amiss::Answering),
    (NEWEST_UNKEPT, Amiss::Answering),
];

/// Place a refusal where the list says its fault lies. A code the list does not hold
/// keeps the place it was raised with.
pub(super) fn place(problem: &mut Problem) {
    if let Some((_, amiss)) = REFUSALS.iter().find(|(code, _)| *code == problem.code) {
        problem.amiss = *amiss;
    }
}

#[cfg(test)]
mod tests;
