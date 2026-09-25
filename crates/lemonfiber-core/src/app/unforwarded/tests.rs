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
