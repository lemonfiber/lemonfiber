//! What running without a forwarded port costs, said where the choice is made.
//!
//! A stack whose provider forwards no port still downloads perfectly well —
//! downloads are connections the client opens, and nothing stops it opening them.
//! What stops is other peers opening connections *to* it, so seeding is slow and
//! some torrents barely seed at all. That is invisible from inside: everything
//! looks healthy, transfers arrive, and only the ratio quietly never moves.
//!
//! So it is said once, at the two moments it is a decision — setting the stack up
//! this way, and changing it to be this way — and never as a recurring warning.
//! A sentence repeated every run about something deliberate is how an operator
//! learns to skim, and the check itself stays skipped precisely because there is
//! nothing here to fix.

use crate::config::{PortForward, Protocols};

/// What no forwarded port costs, in the terms it is felt in.
///
/// Deliberately about seeding rather than about NAT: an operator who reads that
/// downloads still work and seeding does not can decide whether they mind, which
/// is the whole point of saying it.
pub(crate) const COST: &str =
    "No port is forwarded, so other peers cannot open connections to your \
                        torrent client. Downloads still work — those are connections it opens \
                        itself — but peers reach you only when they can, so seeding is slower and \
                        some torrents will barely seed at all.";

/// The consequence to state when a stack is set up this way, or nothing where it
/// costs this stack nothing.
///
/// Nothing where torrents are not configured: a forwarded port buys a Usenet-only
/// stack exactly nothing, and saying it anyway is a sentence about a problem the
/// operator cannot have.
#[must_use]
pub const fn at_setup(protocols: Protocols, port_forward: &PortForward) -> Option<&'static str> {
    if protocols.torrent && !port_forward.enabled {
        Some(COST)
    } else {
        None
    }
}

/// The consequence to state for a change, or nothing where the change does not
/// cost seeding.
///
/// Said when the stack ends up with no port forwarded and the change is what put
/// it there — the switch turned off, or the provider swapped while it is off.
/// Not said where forwarding stays on: whether a port then actually arrives is
/// the runtime check's business, and it says so itself rather than being guessed
/// at from a provider's name.
#[must_use]
pub(crate) fn on_change(before: &PortForward, after: &PortForward) -> Option<&'static str> {
    if after.enabled {
        return None;
    }
    let changed = before.enabled || before.provider != after.provider;
    changed.then_some(COST)
}

#[cfg(test)]
mod tests;
