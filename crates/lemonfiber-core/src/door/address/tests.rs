use super::{address, publishes_a_name, reaches_the_household, Address, MAY_CHANGE};
use crate::platform::Environment;

#[test]
fn a_name_is_preferred_over_the_address_somebody_wrote_down() {
    assert_eq!(
        address(
            Some("kitchen-nas"),
            Some("192.168.1.10"),
            Environment::MacOs,
            5055
        ),
        Some(Address {
            url: "http://kitchen-nas.local:5055".to_owned(),
            caution: None,
        })
    );
}

/// The router's domain is dropped, because the responder is what answers.
///
/// A machine reports itself with whatever suffix came with the lease, and a
/// router that hands out a domain does not necessarily answer for it. Measured
/// against a common home router: `machine.fritz.box` does not resolve at all,
/// while `machine.local` answers — so keeping the domain publishes an address
/// that opens nothing, and does it without looking wrong.
#[test]
fn the_domain_a_router_handed_out_is_dropped_for_the_one_that_answers() {
    assert_eq!(
        address(Some("nas.lan"), None, Environment::Windows, 8096).map(|url| url.url),
        Some("http://nas.local:8096".to_owned())
    );
    assert_eq!(
        address(
            Some("kitchen-nas.fritz.box"),
            None,
            Environment::MacOs,
            5055
        )
        .map(|url| url.url),
        Some("http://kitchen-nas.local:5055".to_owned())
    );
}

/// A name already in the form the responder answers to is left alone.
///
/// Some machines report themselves that way. Appending a second `.local` would
/// ask for a machine nobody has.
#[test]
fn a_name_already_in_the_responders_form_gains_no_second_suffix() {
    assert_eq!(
        address(Some("nas.local"), None, Environment::MacOs, 8096).map(|url| url.url),
        Some("http://nas.local:8096".to_owned())
    );
}

#[test]
fn a_machine_with_no_responder_is_not_offered_by_a_name_that_resolves_nowhere() {
    // A name that fails does not look wrong, which is what makes it worse than
    // a number that says it may change.
    assert!(!publishes_a_name(Environment::LinuxNative));
    assert!(!publishes_a_name(Environment::LinuxDesktop));
    assert!(!publishes_a_name(Environment::Unsupported));
    assert!(publishes_a_name(Environment::MacOs));
    assert!(publishes_a_name(Environment::Windows));
}

#[test]
fn a_number_is_given_with_the_note_that_it_may_change() {
    assert_eq!(
        address(
            Some("kitchen-nas"),
            Some("192.168.1.10"),
            Environment::LinuxNative,
            5055
        ),
        Some(Address {
            url: "http://192.168.1.10:5055".to_owned(),
            caution: Some(MAY_CHANGE.to_owned()),
        })
    );
}

#[test]
fn a_name_the_operator_wrote_down_is_taken_at_its_word() {
    // Their network resolves it, which this cannot check and should not overrule.
    assert_eq!(
        address(None, Some("nas.home.arpa"), Environment::LinuxNative, 5055),
        Some(Address {
            url: "http://nas.home.arpa:5055".to_owned(),
            caution: None,
        })
    );
}

#[test]
fn the_address_this_stack_ships_with_reaches_nobody_and_is_not_offered() {
    // What it ships with means this machine and nowhere else, which is right for
    // a machine nobody has told where it is and wrong to hand anybody.
    assert!(!reaches_the_household("localhost"));
    assert!(!reaches_the_household("LOCALHOST"));
    assert!(!reaches_the_household("127.0.0.1"));
    assert!(!reaches_the_household("::1"));
    assert!(!reaches_the_household("   "));
    assert!(reaches_the_household("192.168.1.10"));
    assert!(reaches_the_household("nas.home.arpa"));
}

#[test]
fn an_address_published_on_every_interface_names_none_of_them() {
    // What the stack's own binding setting defaults to says which interfaces a
    // service is published on, and nothing at all about where to reach it.
    let every = std::net::Ipv4Addr::UNSPECIFIED.to_string();
    assert!(!reaches_the_household(&every));
    assert_eq!(
        address(None, Some(&every), Environment::LinuxNative, 5055),
        None
    );
}

#[test]
fn a_machine_that_says_nothing_and_a_file_that_says_nothing_give_no_address() {
    // Rather than one built from a default, which is an address somebody sends
    // on and nobody can reach.
    assert_eq!(address(None, None, Environment::MacOs, 5055), None);
    assert_eq!(address(Some("  "), None, Environment::MacOs, 5055), None);
    assert_eq!(
        address(Some("kitchen-nas"), None, Environment::LinuxNative, 5055),
        None
    );
}
