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
