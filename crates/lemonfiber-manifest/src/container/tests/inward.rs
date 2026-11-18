//! A service held inside the stack, held to taking no other network namespace.

use super::*;

/// A service held inside the stack runs inside a fence of its own, so it takes no other
/// network namespace beside it, each refused naming the network.
#[test]
fn a_service_held_inside_the_stack_takes_no_other_network_namespace() {
    let none = NONE;
    let inside = Network {
        internal: true,
        ..Network::default()
    };
    let beside = Network {
        via: Some("network.egress-guard".to_owned()),
        fenced: true,
        internal: true,
        egress_for: Some("qbittorrent".to_owned()),
    };
    let mut held = plain(&none);
    held.network = Some(&inside);
    assert!(wrong(&held).is_empty());
    held.network = Some(&beside);
    let said = wrong(&held);
    for what in ["via", "fenced", "egress_for"] {
        assert!(
            said.iter()
                .any(|one| one.contains(&format!("network: says both internal and {what}"))),
            "{said:?}"
        );
    }
    let mut guarding = plain(&none);
    guarding.network = Some(&inside);
    guarding.shape = Some(Shape::EgressGuard);
    assert!(wrong(&guarding)
        .iter()
        .any(|one| one
            .contains("network: says both internal and a shape granting what marks a socket")));
    let hosting = ["network.egress-guard".to_owned()];
    let mut joined = plain(&none);
    joined.network = Some(&inside);
    joined.provides = &hosting;
    assert!(wrong(&joined).iter().any(|one| one.contains(
        "network: says both internal and provides a capability another service runs inside the network of"
    )));
    let read: Result<Network, _> = toml::from_str("internal = true");
    assert_eq!(read.ok(), Some(inside));
}
