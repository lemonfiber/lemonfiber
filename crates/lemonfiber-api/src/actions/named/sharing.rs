//! The requests the line is declared with, and handed back with.
//!
//! Apart from the table that names it for the reason the household's requests are:
//! none of the seven fields it reads is one that table reads, and every one of them
//! would otherwise be a binding taken off the carrier in front of every other row.
//! Pausing every download client and letting them go again read nothing at all, and
//! are here because they are the line's too.
//!
//! **All seven are optional, and all seven absent is the request every surface makes
//! first.** Asked nothing, this is the account of the line — what it carries, what
//! the stack is taking of it, and whether the clients are keeping to that. Given any
//! of them it is a declaration instead. So there is nothing here to be missing, and
//! nothing for this file to refuse: what a value has to *be* is read in the core,
//! where one answer serves the command line and this surface alike.

use lemonfiber_core::app::{BandwidthAsked, Command};
use lemonfiber_core::bandwidth::Pausing;

use crate::actions::Arguments;

/// The action the line is declared with.
const DECLARING: &str = "bandwidth";

/// The action every download client is paused with.
const PAUSING: &str = "downloads-pause";

/// The action every download client is let go again with.
const RESUMING: &str = "downloads-resume";

/// Whether an action is one of the line's.
pub(super) fn about_the_line(action: &str) -> bool {
    [DECLARING, PAUSING, RESUMING].contains(&action)
}

/// The command an action of the line's names.
///
/// The seven a declaration reads are handed on unread. A surface that decided here what
/// `50%` or `07:00-23:00` meant would be a second answer to a question the core already
/// answers, and the two would part company on the first change to either.
pub(super) fn asked_for(action: &str, given: Arguments) -> Command {
    match action {
        PAUSING => return Command::Downloads(Pausing::Pause),
        RESUMING => return Command::Downloads(Pausing::Resume),
        _ => {}
    }
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
mod tests;
