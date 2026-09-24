use lemonfiber_core::app::Command;

use super::{may, Permitted};
use crate::admission::Caller;

/// The member asking, by the id the media server files them under.
const ASKING: &str = "a7f3";

/// Somebody else in the same household, who the one asking is not.
const SOMEBODY_ELSE: &str = "b2e9";

fn member() -> Caller {
    Caller::Member(ASKING.to_owned())
}

#[test]
fn a_members_household_read_is_narrowed_to_them() {
    assert_eq!(
        may(&member(), Command::Household { member: None }),
        Permitted::This(Command::Household {
            member: Some(ASKING.to_owned())
        })
    );
}

/// The one that matters. A request naming another member is not compared and
/// refused; it is answered with the row belonging to whoever is asking — so
/// there is no command in existence that asks the core for somebody else's row
/// on a member's behalf, which is a stronger thing than refusing to send one.
#[test]
fn a_member_naming_somebody_else_is_still_narrowed_to_themselves() {
    assert_eq!(
        may(
            &member(),
            Command::Household {
                member: Some(SOMEBODY_ELSE.to_owned())
            }
        ),
        Permitted::This(Command::Household {
            member: Some(ASKING.to_owned())
        })
    );
}

/// The shelf is narrowed the same way, and the point is the same one: a member's
/// shelf is what their account may watch, so a request naming somebody else is
/// answered with their own rather than compared and turned down.
#[test]
fn a_member_naming_somebody_elses_shelf_is_given_their_own() {
    assert_eq!(
        may(
            &member(),
            Command::Held {
                member: SOMEBODY_ELSE.to_owned(),
                most: 25,
            }
        ),
        Permitted::This(Command::Held {
            member: ASKING.to_owned(),
            most: 25,
        })
    );
}

/// How much of the shelf to answer with is the caller's and survives the
/// narrowing. Whose shelf it is never was theirs to choose, and does not.
#[test]
fn how_much_of_the_shelf_a_member_asked_for_is_carried_through() {
    assert_eq!(
        may(
            &member(),
            Command::Held {
                member: ASKING.to_owned(),
                most: 7,
            }
        ),
        Permitted::This(Command::Held {
            member: ASKING.to_owned(),
            most: 7,
        }),
        "the count a member asked for was not carried through"
    );
}

#[test]
fn a_member_is_refused_a_read_that_is_not_theirs() {
    assert_eq!(may(&member(), Command::Version), Permitted::Nothing);
}

#[test]
fn a_member_is_refused_something_that_changes_the_machine() {
    assert_eq!(may(&member(), Command::AtBoot), Permitted::Nothing);
}

/// Both of the people this product already answered everything for keep the
/// whole surface, and keep the member they named rather than having one chosen
/// for them — an operator reading one person's requests is the point of the
/// parameter.
#[test]
fn the_machine_and_the_operator_are_asked_nothing_new() {
    for caller in [Caller::Machine, Caller::Operator] {
        assert_eq!(
            may(&caller, Command::Version),
            Permitted::This(Command::Version)
        );
        assert_eq!(
            may(
                &caller,
                Command::Household {
                    member: Some(SOMEBODY_ELSE.to_owned())
                }
            ),
            Permitted::This(Command::Household {
                member: Some(SOMEBODY_ELSE.to_owned())
            })
        );
    }
}
