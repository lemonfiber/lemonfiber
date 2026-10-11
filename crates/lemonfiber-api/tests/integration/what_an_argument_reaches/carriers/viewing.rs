//! Where a member's own requests carry what they were given.

use super::super::acting::{DEVICE, POSITION, SEASON, TITLE};
use lemonfiber_api::actions::Arguments;
use lemonfiber_core::app::{Command, Viewing, Wishing};
use lemonfiber_core::ports::media::Kind;

pub(super) fn give_device(given: &mut Arguments) {
    given.device = Some(DEVICE.to_owned());
}

/// Whether the command has the device a grant opens a session for in it.
pub(super) fn carries_device(command: &Command) -> bool {
    matches!(command, Command::Viewing(Viewing::Grant { device, .. }) if device == DEVICE)
}

pub(super) fn give_title(given: &mut Arguments) {
    given.id = Some(TITLE.to_owned());
}

/// Whether the command has the title a player reported on, or a member asked for, in it.
pub(super) fn carries_title(command: &Command) -> bool {
    matches!(command, Command::Viewing(Viewing::Watched { id, .. }) if id == TITLE)
        || matches!(command, Command::Wish(Wishing::Ask { wish, .. }) if wish.id == TITLE)
}

pub(super) fn give_kind(given: &mut Arguments) {
    given.kind = Some("movies".to_owned());
}

/// Whether the command has the kind of title a member asked for in it, and not the kind
/// every action is given by default.
pub(super) fn carries_kind(command: &Command) -> bool {
    matches!(command, Command::Wish(Wishing::Ask { wish, .. }) if wish.kind == Kind::Movies)
}

pub(super) fn give_seasons(given: &mut Arguments) {
    given.seasons = vec![SEASON + 1];
}

/// Whether the command has the seasons a member asked for in it.
pub(super) fn carries_seasons(command: &Command) -> bool {
    matches!(command, Command::Wish(Wishing::Ask { wish, .. }) if wish.seasons == [SEASON + 1])
}

pub(super) fn give_position(given: &mut Arguments) {
    given.position = Some(POSITION + 1);
}

/// Whether the command has how far the member got in it.
pub(super) fn carries_position(command: &Command) -> bool {
    matches!(command, Command::Viewing(Viewing::Watched { how_far, .. })
        if how_far.position == POSITION + 1)
}

pub(super) fn give_ended(given: &mut Arguments) {
    given.ended = Some(true);
}

/// Whether the command has that the member finished it in it.
pub(super) fn carries_ended(command: &Command) -> bool {
    matches!(command, Command::Viewing(Viewing::Watched { how_far, .. }) if how_far.ended)
}
