//! Which setting a command cannot do its work without, where that setting is off.
//!
//! A request whose reach the operator switched off is still a request this stack has:
//! it is refused until the setting is turned back on, and naming the setting is the
//! whole of what a surface needs to say so. Only a command refused outright while its
//! setting is off is named here. One that goes on without what the setting covers, as
//! a start goes on with the images already here, is not unconfigured: it does
//! something, and it says what it left out.

use crate::config::REACH_REGISTRY_KEY;

use super::{Command, Ctx};

/// The setting `command` is refused without, where it is off on this machine.
#[must_use]
pub fn off(ctx: &Ctx, command: &Command) -> Option<&'static str> {
    needed(command).filter(|setting| !ctx.settings.reaching.allows(setting))
}

/// The setting `command` cannot do its work without, whether or not it is on.
///
/// A fetch asks a registry for images and for nothing else, so with
/// [`REACH_REGISTRY_KEY`] off it is refused.
const fn needed(command: &Command) -> Option<&'static str> {
    match command {
        Command::Pull { .. } => Some(REACH_REGISTRY_KEY),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
