use std::net::SocketAddr;

use super::{address, held, permitted, unauthenticated, unavailable, wanted, Offered, Reach};

/// Every reading of the policy, written out.
///
/// As the whole table rather than as the two cases that are interesting, because
/// what makes this a policy rather than a condition is that there is an answer
/// for each of them and only one of them is a refusal.
#[test]
fn the_policy_refuses_one_of_its_four_readings_and_no_other() {
    assert_eq!(permitted(Reach::Machine, false), Offered::Machine);
    assert_eq!(permitted(Reach::Machine, true), Offered::Machine);
    assert_eq!(permitted(Reach::Network, true), Offered::Network);
    assert_eq!(permitted(Reach::Network, false), Offered::Refused);
}

/// Asking for nothing in particular is this machine.
#[test]
fn what_is_asked_for_where_nothing_says_otherwise_is_this_machine() {
    assert_eq!(Reach::default(), Reach::Machine);
}

/// A tier names one address on each family, so a policy cannot be enforced on
/// one of them and be silently absent on the other.
#[test]
fn each_tier_names_an_address_on_both_families() {
    for offered in [Offered::Machine, Offered::Network] {
        let named = wanted(offered, None);
        assert!(named.iter().any(SocketAddr::is_ipv6), "{offered:?}");
        assert!(named.iter().any(SocketAddr::is_ipv4), "{offered:?}");
    }
}

/// This machine's tier names only addresses this machine answers, and the
/// network's names only ones that are not.
#[test]
fn the_addresses_a_tier_names_are_the_ones_that_tier_means() {
    assert!(wanted(Offered::Machine, None)
        .iter()
        .all(|address| address.ip().is_loopback()));
    assert!(wanted(Offered::Refused, None)
        .iter()
        .all(|address| address.ip().is_loopback()));
    assert!(wanted(Offered::Network, None)
        .iter()
        .all(|address| address.ip().is_unspecified()));
}

#[test]
fn naming_no_port_asks_for_whichever_one_is_free() {
    assert!(wanted(Offered::Machine, None)
        .iter()
        .all(|address| address.port() == 0));
}

#[test]
fn naming_a_port_asks_for_that_one_on_every_family() {
    assert!(wanted(Offered::Machine, Some(7171))
        .iter()
        .all(|address| address.port() == 7171));
}

#[tokio::test]
async fn every_address_taken_is_this_machine_and_they_share_one_port() {
    // Asserted as one value rather than through a branch on the way in: this
    // module's tests are under the same coverage gate as the code, and an arm
    // for a bind that never fails is a line nothing could ever run.
    let taken = held(Offered::Machine, None).await.ok();
    let at: Vec<SocketAddr> = taken.iter().flatten().map(|(_, bound)| *bound).collect();
    let first = at.first().map_or(0, SocketAddr::port);
    assert!(!at.is_empty(), "not one address could be taken");
    assert!(at.iter().all(|bound| bound.ip().is_loopback()), "{at:?}");
    assert!(at.iter().all(|bound| bound.port() == first), "{at:?}");
    assert_ne!(first, 0, "a port nobody named is one the machine settled");
}

#[tokio::test]
async fn an_address_no_family_can_take_is_reported_rather_than_swapped() {
    // A port this run already holds, asked for again by name. Both families are
    // asked for it and neither can have it, which is the one case that is a
    // failure rather than one fewer socket.
    let taken = held(Offered::Machine, None).await.ok();
    let port = taken.iter().flatten().next().map(|(_, bound)| bound.port());
    let again = held(Offered::Machine, port).await;
    assert!(
        again
            .err()
            .is_some_and(|problem| problem.summary.contains("could not start serving")),
        "a port this run is holding is not one it can take again"
    );
    drop(taken);
}

#[test]
fn a_machine_with_no_free_port_at_all_says_that_instead() {
    // The other half of the same fault: asking for any port and being given none
    // says something different from being refused a named one.
    let any = unavailable(wanted(Offered::Machine, None), "denied").summary;
    let named = unavailable(wanted(Offered::Machine, Some(7171)), "denied").summary;
    assert!(any.contains("no free port"), "{any}");
    assert!(named.contains("127.0.0.1:7171"), "{named}");
    assert!(named.contains("[::1]:7171"), "{named}");
}

#[test]
fn a_refusal_to_take_an_address_offers_both_ways_out() {
    // Ask for another port, or stop asking for one in particular.
    let problem = unavailable(wanted(Offered::Machine, Some(7171)), "address in use");
    assert_eq!(problem.remedies.len(), 2);
    assert_eq!(problem.detail.as_deref(), Some("address in use"));
}

/// The network without a password is refused, and the refusal says how.
///
/// Both ways out, because there are two: set one, or stop asking for the network.
/// A refusal naming neither is a refusal somebody has to go and look things up
/// after.
#[test]
fn refusing_the_network_says_how_to_be_allowed_it() {
    let problem = unauthenticated();
    assert_eq!(problem.remedies.len(), 2);
    assert!(
        problem.remedies.iter().any(|remedy| remedy
            .detail
            .as_deref()
            .is_some_and(|said| said.contains("--set-password"))),
        "{problem:?}"
    );
}

#[test]
fn the_address_is_printed_whole() {
    assert_eq!(
        address(SocketAddr::from(([127, 0, 0, 1], 8471))),
        "http://127.0.0.1:8471"
    );
}
