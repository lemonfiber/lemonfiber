//! A member's choice of where they are told, and a phone's word that a wake arrived.
//!
//! Apart from the table that names the rest because each is a member's or a phone's own:
//! a member's choice is narrowed to them whatever it names, and a wake is confirmed only
//! by whoever registered the phone it woke.

use lemonfiber_core::app::Command;
use lemonfiber_core::notify::configuring::Asked as Notifying;
use lemonfiber_core::push::run::Asked as Pushing;

use crate::actions::{Arguments, Refused};

/// The actions this file answers.
const ADDRESSED: [&str; 2] = ["notify-choose", "push-woken"];

/// Whether this action is one of them.
#[must_use]
pub(super) fn about_notifying(action: &str) -> bool {
    ADDRESSED.contains(&action)
}

/// The command one of them names, or why it names none.
pub(super) fn asked_for(action: &str, given: Arguments) -> Result<Command, Refused> {
    let Arguments {
        name, id, backends, ..
    } = given;
    let missing = |argument: &str| Refused::Missing {
        action: action.to_owned(),
        argument: argument.to_owned(),
    };
    if action == "notify-choose" {
        return Ok(Command::Notify(Notifying::Choose {
            member: String::new(),
            backends,
        }));
    }
    Ok(Command::PushEndpoints(Pushing::Woken {
        name: name.ok_or_else(|| missing("name"))?,
        id: id.ok_or_else(|| missing("id"))?,
        owner: None,
    }))
}
