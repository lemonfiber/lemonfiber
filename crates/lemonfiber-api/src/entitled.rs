//! What one caller may ask for, decided on the command rather than on the route.
//!
//! The guard says who is knocking. This says what that person may have, and it is
//! the only place that decides — a surface drawing its own conclusion from the same
//! answer would be a second opinion able to disagree with this one, and the one that
//! disagreed would be the one nobody tested.
//!
//! **Asked of the command, not of the path.** A path is an alias for a command, and
//! a rule written against the alias is a rule that can drift from what the command
//! actually does: a route renamed, a second route reaching the same command, or a
//! command growing a new effect all leave a path list saying something that was true
//! when it was written. What runs is the command, so what is ruled on is the command.
//!
//! **A member's read is narrowed by construction rather than filtered afterwards.**
//! The command that leaves here names the member who asked, whatever the request
//! named — so there is no path on which the core is asked for somebody else's row,
//! let alone sends one. Filtering afterwards would read every row and rely on the
//! device to draw one of them, which is a promise about a screen where this is a
//! fact about the wire. That is what `N3-R9` asks for.

use lemonfiber_core::app::Command;

use crate::admission::Caller;

/// What is left of a command once who is asking has been taken into account.
///
/// A type of its own rather than an `Option<Command>`, because `Option` carries
/// combinators — `unwrap_or`, `unwrap_or_default` — that would turn a refusal back
/// into the command that was refused, in one word, while reading like tidying up.
/// There is no such word for this, so the only way past it is to write the arm.
#[derive(Debug, PartialEq, Eq)]
pub enum Permitted {
    /// Carry this out. It may not be the command that was asked for: a command
    /// narrowed to its caller arrives here as the narrowed one.
    This(Command),
    /// Nothing, and say so. The caller proved who they are and this is not theirs.
    Nothing,
}

/// The command this caller actually gets, or nothing.
///
/// Every command reaches this, whether it was asked for as a read, at the actions
/// door or as a step of setup, and whether it is answered now or handed to a job —
/// so a control a member is not entitled to is refused by the core rather than by
/// the app having omitted it, which is what `N3-R3` asks for. A hand-written
/// request gets no further here than a tapped button does.
#[must_use]
pub fn may(caller: &Caller, command: Command) -> Permitted {
    match caller {
        // The whole surface, unchanged. Somebody at this machine's terminal and
        // somebody holding this machine's password are the two people this product
        // already answered everything for, and nothing here narrows that.
        Caller::Machine | Caller::Operator => Permitted::This(command),
        // **Everything not named below is refused**, and the catch-all is the
        // statement rather than an omission: a command added later is not a
        // member's until somebody decides it is and writes it down. Listing what
        // members may *not* do would make every new command theirs by default, and
        // the day that is wrong is the day nobody notices.
        Caller::Member(id) => match command {
            // Theirs, narrowed to them. Whatever the request named is discarded
            // rather than compared, so there is no arm on which a mismatch could be
            // let through — and a page left open reloading with a stale name is
            // answered with their own row rather than signed out for holding it.
            Command::Household { .. } => Permitted::This(Command::Household {
                member: Some(id.clone()),
            }),
            _ => Permitted::Nothing,
        },
    }
}

#[cfg(test)]
mod tests {
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
}
