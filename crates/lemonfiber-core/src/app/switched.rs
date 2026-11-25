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
    command
        .needed_setting()
        .filter(|setting| !ctx.settings.reaching.allows(setting))
}

/// Each setting some command cannot do its work without that is off on this machine.
pub(crate) fn switched_off(ctx: &Ctx) -> Vec<String> {
    NEEDED
        .iter()
        .filter(|setting| !ctx.settings.reaching.allows(setting))
        .map(|setting| (*setting).to_owned())
        .collect()
}

/// Every setting some command cannot do its work without.
const NEEDED: [&str; 1] = [REACH_REGISTRY_KEY];

impl Command {
    /// The setting this command cannot do its work without, whether or not it is on.
    ///
    /// A fetch asks a registry for images and for nothing else, so with
    /// [`REACH_REGISTRY_KEY`] off it is refused.
    #[must_use]
    pub const fn needed_setting(&self) -> Option<&'static str> {
        match self {
            Command::Pull { .. } => Some(REACH_REGISTRY_KEY),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
