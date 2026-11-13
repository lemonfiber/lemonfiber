//! Where a member's own requests carry what they were given.

use super::super::acting::{DEVICE, POSITION, SEASON, TITLE};
use lemonfiber_api::actions::{Arguments, Handed};
use lemonfiber_core::app::{Command, Live, PlayFrom, Viewing, Wishing};
use lemonfiber_core::notify::configuring::Asked as Notifying;
use lemonfiber_core::ports::media::Kind;
use lemonfiber_core::push::run::Asked as Pushing;

pub(super) fn give_device(given: &mut Arguments) {
    given.device = Some(DEVICE.to_owned());
}

/// Whether the command has the device a grant or a browser's play opens a session for
/// in it.
pub(super) fn carries_device(command: &Command) -> bool {
    matches!(command, Command::Viewing(Viewing::Grant { device, .. }) if device == DEVICE)
        || matches!(command, Command::Viewing(Viewing::Play {
            from: PlayFrom::Device(device),
            ..
        }) if device == DEVICE)
}

/// A live stream an earlier answer named.
const STREAM: &str = "ls1";

pub(super) fn give_stream(given: &mut Arguments) {
    given.stream = Some(STREAM.to_owned());
}

/// Whether the command renews the live stream named.
pub(super) fn carries_stream(command: &Command) -> bool {
    matches!(command, Command::Viewing(Viewing::Tune {
        live: Live::Renew(stream),
        ..
    }) if stream == STREAM)
}

pub(super) fn give_stop(given: &mut Arguments) {
    given.stop = Some(true);
}

/// Whether the command closes a live stream.
pub(super) fn carries_stop(command: &Command) -> bool {
    matches!(
        command,
        Command::Viewing(Viewing::Tune {
            live: Live::Stop(_),
            ..
        })
    )
}

/// A pass a browser holds.
const PASS: &str = "bm9uY2U.1800000300.c2VhbGVk";

pub(super) fn give_pass(given: &mut Arguments) {
    given.pass = Some(Handed(PASS.to_owned()));
}

/// Whether the command holds the pass a browser handed back.
pub(super) fn carries_pass(command: &Command) -> bool {
    matches!(command, Command::Viewing(Viewing::Play {
        from: PlayFrom::Pass(pass),
        ..
    }) if pass == PASS)
}

pub(super) fn give_title(given: &mut Arguments) {
    given.id = Some(TITLE.to_owned());
}

/// Whether the command has the title a player reported on, a member asked for, or the
/// wake a phone confirms, in it.
pub(super) fn carries_title(command: &Command) -> bool {
    matches!(command, Command::Viewing(Viewing::Watched { id, .. }) if id == TITLE)
        || matches!(command, Command::Viewing(Viewing::Play { id, .. }) if id == TITLE)
        || matches!(command, Command::Viewing(Viewing::Tune {
            live: Live::Open(id),
            ..
        }) if id == TITLE)
        || matches!(command, Command::Wish(Wishing::Ask { wish, .. }) if wish.id == TITLE)
        || matches!(command, Command::PushEndpoints(Pushing::Woken { id, .. }) if id == TITLE)
}

pub(super) fn give_backends(given: &mut Arguments) {
    given.backends = vec!["another-way".to_owned()];
}

/// Whether the command has the ways to be told a member chose in it.
pub(super) fn carries_backends(command: &Command) -> bool {
    matches!(command, Command::Notify(Notifying::Choose { backends, .. })
        if backends == &["another-way".to_owned()])
}

pub(super) fn give_kind(given: &mut Arguments) {
    given.kind = Some("movies".to_owned());
}

/// Whether the command has the kind of title a member asked for in it, and not the kind
/// every action is given by default.
pub(super) fn carries_kind(command: &Command) -> bool {
    matches!(command, Command::Wish(Wishing::Ask { wish, .. }) if wish.kind == Kind::Movies)
}

pub(super) fn give_unit(given: &mut Arguments) {
    if given.kind.is_some() {
        given.kind = Some("music".to_owned());
        given.seasons = Vec::new();
    }
    given.unit = Some("artist".to_owned());
}

/// Whether the command asks for the unit of music a member named.
pub(super) fn carries_unit(command: &Command) -> bool {
    matches!(
        command,
        Command::Wish(Wishing::Collect {
            unit: lemonfiber_core::collected::Unit::Music(
                lemonfiber_core::ports::service::MusicUnit::Artist
            ),
            ..
        })
    )
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
