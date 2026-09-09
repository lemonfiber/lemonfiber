//! Telling a hand-edit to the configuration file from what lemonfiber put there.
//!
//! The environment file is a file an operator edits, deliberately: comments and
//! ordering are preserved on every write for exactly that reason. So a change made
//! through lemonfiber may be about to write over one made outside it, and the two
//! are indistinguishable from the file alone — the value is simply a value.
//!
//! Three values tell them apart where two cannot: what lemonfiber last wrote here,
//! what the file holds now, and what is about to be written. That is the same
//! comparison seeding makes about a service's configuration, so it is the same
//! comparison — reached for rather than written again, because two answers to
//! "whose value is this" that could disagree is worse than none.

use crate::baseline::Record;
use crate::seed::{reconcile, Observed};

/// What the record, the file and the change together say about a setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// The file already holds what is about to be written; there is nothing to do.
    Already,
    /// The file holds what lemonfiber last wrote there, so the change is
    /// lemonfiber's own to make.
    Ours,
    /// lemonfiber has no record of ever writing here, so an operator's edit cannot
    /// be told from the value setup itself left. What is there is taken as the
    /// starting point rather than judged against an expectation nobody formed.
    Unrecorded,
    /// The file was changed outside lemonfiber since it last wrote here.
    Edited,
}

/// What `writing` would be doing to this setting, given what lemonfiber recorded
/// and what the file holds now.
///
/// A setting missing from the file that lemonfiber has a record for reads as an
/// edit, because it is one: somebody took the line out.
#[must_use]
pub fn standing(recorded: Option<&Record>, found: Option<&str>, writing: &str) -> Standing {
    match reconcile(recorded, found, writing) {
        Observed::Present => Standing::Already,
        Observed::Stale => Standing::Ours,
        Observed::Unmanaged => Standing::Unrecorded,
        _ => Standing::Edited,
    }
}

#[cfg(test)]
mod tests {
    use super::{standing, Standing};
    use crate::baseline::{Origin, Record};

    /// What lemonfiber last wrote here, as the baseline holds it.
    fn wrote(value: &str) -> Record {
        Record {
            value: value.to_owned(),
            at: "0".to_owned(),
            origin: Origin::Written,
        }
    }

    #[test]
    fn a_file_still_holding_what_lemonfiber_wrote_is_lemonfibers_to_change() {
        assert_eq!(
            standing(Some(&wrote("/srv/old")), Some("/srv/old"), "/srv/new"),
            Standing::Ours
        );
    }

    #[test]
    fn a_file_changed_under_lemonfiber_is_an_edit_rather_than_a_stale_value() {
        // The whole of it: the operator moved this by hand, and a change that
        // overwrote it without saying so would take their edit with it.
        assert_eq!(
            standing(Some(&wrote("/srv/old")), Some("/mnt/theirs"), "/srv/new"),
            Standing::Edited
        );
    }

    #[test]
    fn a_line_taken_out_of_the_file_is_an_edit_too() {
        assert_eq!(
            standing(Some(&wrote("/srv/old")), None, "/srv/new"),
            Standing::Edited
        );
    }

    #[test]
    fn a_setting_lemonfiber_never_wrote_is_not_judged_against_an_expectation() {
        // Without a record there is no third value, and calling what setup wrote an
        // operator's edit would refuse every first change on a fresh install.
        assert_eq!(
            standing(None, Some("/srv/old"), "/srv/new"),
            Standing::Unrecorded
        );
    }

    #[test]
    fn a_file_that_already_holds_the_new_value_has_nothing_to_overwrite() {
        assert_eq!(
            standing(Some(&wrote("/srv/old")), Some("/srv/new"), "/srv/new"),
            Standing::Already
        );
        assert_eq!(
            standing(None, Some("/srv/new"), "/srv/new"),
            Standing::Already
        );
    }

    #[test]
    fn a_value_lemonfiber_adopted_from_the_operator_stays_theirs() {
        // Adopted rather than written: the operator set it, lemonfiber took it on,
        // and a change over it is still a change over theirs.
        let adopted = Record {
            value: "/mnt/theirs".to_owned(),
            at: "0".to_owned(),
            origin: Origin::Adopted,
        };
        assert_eq!(
            standing(Some(&adopted), Some("/mnt/theirs"), "/srv/new"),
            Standing::Edited
        );
    }
}
