//! The requests a member's own client makes when they sit down to watch.
//!
//! Apart from the table that names the rest because each is a member's: a grant for a
//! device to play on their account, how far they got, a channel's live stream and a
//! title played in their browser. A member's request is narrowed to them whatever it
//! names; the operator names the member, and naming nobody is refused by the core,
//! which knows who is in the household.

use lemonfiber_core::app::{Asker, Command, Live, PlayFrom, Viewing};
use lemonfiber_core::ports::service::HowFar;

use crate::actions::{Arguments, Handed, Refused};

/// The requests this file answers.
const ADDRESSED: [&str; 4] = ["grant", "watched", "tune", "play"];

/// Whether this action is one of them.
#[must_use]
pub(super) fn about_viewing(action: &str) -> bool {
    ADDRESSED.contains(&action)
}

/// The command one of them names, or why it names none.
pub(super) fn asked_for(action: &str, given: Arguments) -> Result<Command, Refused> {
    let Arguments {
        name,
        device,
        id,
        position,
        ended,
        stream,
        stop,
        pass,
        ..
    } = given;
    let member = name.unwrap_or_default();
    let missing = |argument: &str| Refused::Missing {
        action: action.to_owned(),
        argument: argument.to_owned(),
    };
    let together = |argument: &str, alongside: &str| Refused::Together {
        action: action.to_owned(),
        argument: argument.to_owned(),
        alongside: alongside.to_owned(),
    };
    let viewing = match action {
        "grant" => Viewing::Grant {
            member,
            device: device.ok_or_else(|| missing("device"))?,
        },
        "tune" => Viewing::Tune {
            member,
            live: match (id, stream, stop.unwrap_or(false)) {
                (Some(_), Some(_), _) => return Err(together("id", "stream")),
                (Some(_), None, true) => return Err(together("id", "stop")),
                (Some(channel), None, false) => Live::Open(channel),
                (None, Some(stream), false) => Live::Renew(stream),
                (None, Some(stream), true) => Live::Stop(stream),
                (None, None, _) => return Err(missing("id")),
            },
            asker: Asker::Operator,
        },
        "play" => Viewing::Play {
            member,
            id: id.ok_or_else(|| missing("id"))?,
            from: match (device, pass) {
                (Some(_), Some(_)) => return Err(together("device", "pass")),
                (Some(device), None) => PlayFrom::Device(device),
                (None, Some(Handed(pass))) => PlayFrom::Pass(pass),
                (None, None) => return Err(missing("device")),
            },
            asker: Asker::Operator,
        },
        _ => Viewing::Watched {
            member,
            id: id.ok_or_else(|| missing("id"))?,
            how_far: HowFar {
                position: position.ok_or_else(|| missing("position"))?,
                ended: ended.unwrap_or(false),
            },
        },
    };
    Ok(Command::Viewing(viewing))
}
