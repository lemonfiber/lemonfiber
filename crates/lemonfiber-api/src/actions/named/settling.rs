//! The acts that change what the stack is set to: a configuration key, and a setting a
//! service's environment reads.
//!
//! Apart from the table that names them for the reason the quality choice is: three
//! rows reading fields no other row reads, where a missing subject is the only thing any
//! of them can refuse. Whether a name is a setting, a secret or a configuration key is
//! the core's answer.

use lemonfiber_core::app::{Command, Setting, Settling};

use super::{Arguments, Refused};

/// Every act on what the stack is set to.
const ABOUT: [&str; 3] = ["config-set", "settings-set", "settings-clear"];

/// Whether this action changes what the stack is set to.
pub(super) fn about_settings(action: &str) -> bool {
    ABOUT.contains(&action)
}

/// What the action asks the core for, or why it asks nothing.
///
/// A value is carried as exactly what was written, an empty one included, because a
/// setting may be set to nothing. A setting sent blank names none.
///
/// # Errors
///
/// Where the key, the setting or the value the action needs was not given.
pub(super) fn asked_for(action: &str, given: Arguments) -> Result<Command, Refused> {
    let Arguments {
        key,
        value,
        setting,
        offer,
        confirm,
        wait,
        ..
    } = given;
    let needs = |argument: &str| Refused::Missing {
        action: action.to_owned(),
        argument: argument.to_owned(),
    };
    if action == "config-set" {
        let key = key.ok_or_else(|| needs("key"))?;
        let value = value.ok_or_else(|| needs("value"))?;
        return Ok(Command::ConfigSet(
            Setting::to(&key, &value).agreed(confirm).waiting(wait),
        ));
    }
    let setting = setting
        .filter(|named| !named.trim().is_empty())
        .ok_or_else(|| needs("setting"))?;
    let agreement = offer.filter(|named| !named.trim().is_empty());
    let settling = if action == "settings-set" {
        Settling::Set {
            setting,
            value: value.ok_or_else(|| needs("value"))?,
            agreement,
        }
    } else {
        Settling::Clear { setting, agreement }
    };
    Ok(Command::Settings(settling))
}
