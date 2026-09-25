use super::{Holding, Left};

/// Taking nothing is told from taking something, which is what decides whether
/// anybody is written down as held back at all.
#[test]
fn taking_nothing_is_not_a_member_held_back() {
    let nothing = Holding::default();
    let something = Holding { taken: 32 };

    assert!(!nothing.anything());
    assert!(something.anything());
    // Formatted rather than left to an assertion's message, which is evaluated only
    // where the assertion fails: what carries the service's own number has to be
    // readable in a report, and a rendering nothing runs is not.
    assert_eq!(format!("{something:?}"), "Holding { taken: 32 }");
}

/// What is left is the limit less what has been spent.
#[test]
fn what_is_left_is_the_limit_less_what_was_spent() {
    let held = Left {
        limit: Some(5),
        used: 2,
        days: Some(7),
    };

    assert_eq!(held.remaining(), Some(3));
    assert!(!held.spent());
}

/// A member nothing limits has no number left, which is not nought left.
///
/// Nought would be somebody who may ask for nothing. Reported as the same figure,
/// an unlimited member would read as the most restricted one in the house.
#[test]
fn a_member_nothing_limits_has_no_figure_rather_than_nought() {
    let open = Left {
        limit: None,
        used: 40,
        days: None,
    };

    assert_eq!(open.remaining(), None);
    assert!(!open.spent());
}

/// A limit lowered under what is already spent leaves nought, never a wrap.
#[test]
fn a_limit_lowered_under_what_is_spent_leaves_nought() {
    let over = Left {
        limit: Some(2),
        used: 9,
        days: Some(30),
    };

    assert_eq!(over.remaining(), Some(0));
    assert!(over.spent());
}
