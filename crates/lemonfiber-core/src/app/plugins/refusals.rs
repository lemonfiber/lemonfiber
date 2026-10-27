//! Every refusal of an install, an update, a removal, a proving or a revoked grant, listed so
//! a client can
//! name each code and read its status before it meets it. The status is the one each
//! code is declared with in the registry.
//!
//! The record that cannot be read and the offer that moved are listed with the reads
//! and with every other moved offer, so they are not listed twice. A machine setup has
//! not run on is refused with the configuration's own code, as every record refuses it,
//! and is not a plugin's to list.

use crate::error::codes::plugin::{
    ADDRESS_REFUSED, ALREADY, ANOTHER_PLUGIN, ANSWERED, CALL_REFUSED, CATALOGUE_OFF,
    CATALOGUE_REPLACED, CATALOGUE_UNREACHABLE, CATALOGUE_UNREADABLE, EARLIER_TAKEN, HEADER_NAMED,
    INPUT_UNMATCHED, MANIFEST_UNKEPT, NEWEST_UNKEPT, NOTHING_TO_PROVE, NOTHING_TO_REMOVE,
    NOTHING_TO_RUN, NOTHING_TO_UPDATE, NOT_AS_REVIEWED, NOT_CATALOGUED, NOWHERE, NO_GRANT,
    NO_REVISION, OCCUPIED, PATH_NOT_PLAIN, REFUSED, SCHEME_REFUSED, SIGNATURE_UNVERIFIED,
    SOURCE_OFF, SPELLED_ALIKE, STEP_FAILED, STRANGER_REPLACING, STUCK, TWO_SOURCES,
    UNAIMED_CROSSING, UNAPPROVED, UNFETCHED, UNPROVED, UNREADABLE, UNRECORDABLE, UNWRITABLE,
    VALUE_WITHHELD,
};
use crate::error::Code;

/// Every plugin code an install, an update, a removal, a proving or a revoked grant is
/// refused with, apart
/// from the record that cannot be read and the offer that moved.
pub const REFUSALS: [Code; 42] = [
    UNREADABLE,
    REFUSED,
    ALREADY,
    NOWHERE,
    UNWRITABLE,
    UNRECORDABLE,
    UNPROVED,
    NOTHING_TO_REMOVE,
    NOTHING_TO_UPDATE,
    STUCK,
    ANSWERED,
    TWO_SOURCES,
    SOURCE_OFF,
    UNFETCHED,
    NO_REVISION,
    CATALOGUE_OFF,
    CATALOGUE_UNREACHABLE,
    SIGNATURE_UNVERIFIED,
    CATALOGUE_UNREADABLE,
    NOT_CATALOGUED,
    NOT_AS_REVIEWED,
    SPELLED_ALIKE,
    UNAPPROVED,
    ANOTHER_PLUGIN,
    OCCUPIED,
    SCHEME_REFUSED,
    ADDRESS_REFUSED,
    HEADER_NAMED,
    INPUT_UNMATCHED,
    CALL_REFUSED,
    STEP_FAILED,
    PATH_NOT_PLAIN,
    VALUE_WITHHELD,
    CATALOGUE_REPLACED,
    NEWEST_UNKEPT,
    STRANGER_REPLACING,
    NOTHING_TO_PROVE,
    NOTHING_TO_RUN,
    MANIFEST_UNKEPT,
    UNAIMED_CROSSING,
    NO_GRANT,
    EARLIER_TAKEN,
];

#[cfg(test)]
mod tests;
