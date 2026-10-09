//! The requests a member's own client makes when they sit down to watch.
//!
//! Apart from the table that names the rest because each is a member's: a grant for a
//! device to play on their account, and how far they got. A member's request is
//! narrowed to them whatever it names; the operator names the member, and naming nobody
//! is refused by the core, which knows who is in the household.

use lemonfiber_core::app::{Command, Viewing};
use lemonfiber_core::ports::service::HowFar;

use crate::actions::{Arguments, Refused};

/// The requests this file answers.
const ADDRESSED: [&str; 2] = ["grant", "watched"];

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
        ..
    } = given;
    let member = name.unwrap_or_default();
    let missing = |argument: &str| Refused::Missing {
        action: action.to_owned(),
        argument: argument.to_owned(),
    };
    let viewing = if action == "grant" {
        Viewing::Grant {
            member,
            device: device.ok_or_else(|| missing("device"))?,
        }
    } else {
        Viewing::Watched {
            member,
            id: id.ok_or_else(|| missing("id"))?,
            how_far: HowFar {
                position: position.ok_or_else(|| missing("position"))?,
                ended: ended.unwrap_or(false),
            },
        }
    };
    Ok(Command::Viewing(viewing))
}
