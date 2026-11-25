//! What the web API composes its routes and its answers from, read in one place.

use serde::Serialize;

use super::Ctx;

/// What the web API composes its routes and its answers from.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct StackStanding {
    /// The stack's own identifier, the one pairing material carries; absent where this
    /// machine has nowhere to keep one.
    pub stack: Option<String>,
    /// Each setting a command is refused without that is off on this machine.
    pub switched_off: Vec<String>,
    /// Whether a way to be told is configured.
    pub told: bool,
    /// Whether a phone's push endpoint is waiting to be woken.
    pub waiting: bool,
    /// Whether household members may mint keys for themselves.
    pub members_mint: bool,
    /// The origin a browser plays a title from, which the page's policy allows, where
    /// the core knows one: built from the stack's front door, never from a request.
    pub plays_from: Option<crate::screening::Origin>,
}

/// What the web API composes its routes and its answers from, on the machine `ctx` runs.
pub(crate) fn standing(ctx: &Ctx) -> StackStanding {
    StackStanding {
        stack: crate::companion::identified(ctx),
        switched_off: super::switched::switched_off(ctx),
        told: crate::notify::fanning::any_configured(ctx),
        waiting: crate::push::run::any_waiting(ctx),
        members_mint: crate::keys::run::members_may_mint(ctx),
        plays_from: crate::screening::plays_from(ctx),
    }
}

#[cfg(test)]
mod tests;
