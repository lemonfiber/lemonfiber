//! The request the line is declared with.
//!
//! Apart from the table that names it for the reason the household's requests are:
//! none of the seven fields it reads is one that table reads, and every one of them
//! would otherwise be a binding taken off the carrier in front of every other row.
//!
//! **All seven are optional, and all seven absent is the request every surface makes
//! first.** Asked nothing, this is the account of the line — what it carries, what
//! the stack is taking of it, and whether the clients are keeping to that. Given any
//! of them it is a declaration instead. So there is nothing here to be missing, and
//! nothing for this file to refuse: what a value has to *be* is read in the core,
//! where one answer serves the command line and this surface alike.

use lemonfiber_core::app::{BandwidthAsked, Command};

use crate::actions::Arguments;

/// The action this file answers.
const DECLARING: &str = "bandwidth";

/// Whether an action is the one the line is declared with.
pub(super) fn about_the_line(action: &str) -> bool {
    action == DECLARING
}

/// The command it names, carrying the seven as they were written.
///
/// Each is handed on unread. A surface that decided here what `50%` or `07:00-23:00`
/// meant would be a second answer to a question the core already answers, and the two
/// would part company on the first change to either.
pub(super) fn asked_for(given: Arguments) -> Command {
    Command::Bandwidth(BandwidthAsked {
        down: given.down,
        up: given.up,
        active: given.active,
        line: given.line,
        cap: given.cap,
        exceeded: given.exceeded,
        unrestricted_for: given.unrestricted_for,
    })
}

#[cfg(test)]
mod tests {
    use super::{about_the_line, asked_for};
    use crate::actions::Arguments;
    use lemonfiber_core::app::Command;

    #[test]
    fn only_the_one_action_is_about_the_line() {
        assert!(about_the_line("bandwidth"));
        assert!(!about_the_line("space"));
        assert!(!about_the_line("household"));
    }

    #[test]
    fn nothing_given_is_the_reading_every_surface_makes_first() {
        assert_eq!(
            asked_for(Arguments::default()),
            Command::Bandwidth(lemonfiber_core::app::BandwidthAsked::default())
        );
    }

    #[test]
    fn every_word_reaches_the_command_as_it_was_written() {
        let given = Arguments {
            down: Some("50%".to_owned()),
            up: Some("2MiB".to_owned()),
            active: Some("07:00-23:00".to_owned()),
            line: Some("60MiB/6MiB".to_owned()),
            cap: Some("1TiB".to_owned()),
            exceeded: Some("pause".to_owned()),
            unrestricted_for: Some(90),
            ..Arguments::default()
        };
        assert_eq!(
            asked_for(given),
            Command::Bandwidth(lemonfiber_core::app::BandwidthAsked {
                down: Some("50%".to_owned()),
                up: Some("2MiB".to_owned()),
                active: Some("07:00-23:00".to_owned()),
                line: Some("60MiB/6MiB".to_owned()),
                cap: Some("1TiB".to_owned()),
                exceeded: Some("pause".to_owned()),
                unrestricted_for: Some(90),
            })
        );
    }
}
