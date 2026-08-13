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
pub const COST: &str = "No port is forwarded, so other peers cannot open connections to your \
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
pub fn on_change(before: &PortForward, after: &PortForward) -> Option<&'static str> {
    if after.enabled {
        return None;
    }
    let changed = before.enabled || before.provider != after.provider;
    changed.then_some(COST)
}

#[cfg(test)]
mod tests {
    use super::{at_setup, on_change, COST};
    use crate::config::{PortForward, Protocols};

    /// A forwarding configuration.
    fn forwarding(enabled: bool, provider: Option<&str>) -> PortForward {
        PortForward {
            enabled,
            provider: provider.map(str::to_owned),
        }
    }

    #[test]
    fn a_torrent_stack_with_no_forwarded_port_is_told_what_it_costs() {
        assert_eq!(
            at_setup(Protocols::both(), &forwarding(false, Some("mullvad"))),
            Some(COST)
        );
    }

    #[test]
    fn a_stack_that_forwards_a_port_is_told_nothing() {
        assert_eq!(
            at_setup(Protocols::both(), &forwarding(true, Some("pia"))),
            None
        );
    }

    #[test]
    fn a_stack_that_does_not_torrent_is_told_nothing_either() {
        // A forwarded port buys a Usenet-only stack nothing, so the sentence would
        // be about a problem this operator cannot have.
        let usenet = Protocols {
            usenet: true,
            torrent: false,
        };
        assert_eq!(at_setup(usenet, &forwarding(false, None)), None);
    }

    #[test]
    fn turning_forwarding_off_states_what_it_costs() {
        assert_eq!(
            on_change(
                &forwarding(true, Some("pia")),
                &forwarding(false, Some("pia"))
            ),
            Some(COST)
        );
    }

    #[test]
    fn moving_to_another_provider_that_forwards_nothing_states_it_again() {
        // The requirement's own case: a provider change that leaves the stack with
        // no forwarded port is a new decision, not the old one repeated.
        assert_eq!(
            on_change(
                &forwarding(false, Some("pia")),
                &forwarding(false, Some("someone-else"))
            ),
            Some(COST)
        );
    }

    #[test]
    fn a_change_that_touches_neither_says_nothing() {
        // Every other setting a stack has passes through here, and a sentence about
        // seeding attached to an unrelated change reads as a warning nobody caused.
        let same = forwarding(false, Some("pia"));
        assert_eq!(on_change(&same, &same), None);
    }

    #[test]
    fn asking_for_a_port_is_never_second_guessed_from_a_provider_name() {
        // Whether a port actually arrives is the runtime check's business. Guessing
        // it from the provider is how an operator gets told their working stack is
        // broken.
        assert_eq!(
            on_change(
                &forwarding(false, Some("pia")),
                &forwarding(true, Some("obscure-provider"))
            ),
            None
        );
    }
}
