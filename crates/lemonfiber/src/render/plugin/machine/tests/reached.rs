//! How each installed service is said to be reached.

use super::installed::listing;
use super::*;

/// A loopback service says so and names no address: lemonfiber renders one from
/// the tier, and a line here spelling one out would be a second answer.
#[test]
fn an_operator_surface_is_shown_as_this_machine_only_and_still_on_the_panel() {
    let said = installs(&Installs {
        demand: None,
        nonconforming: Vec::new(),
        proof: None,
        rehearsed: false,
        agreement: None,
        removal: None,
        installed: vec![recorded(
            "komga",
            Some(Reached::Loopback {
                port: 9000,
                group: Some("Operators".to_owned()),
            }),
        )],
        install: None,
        update: None,
        substituted: Vec::new(),
        sources: Vec::new(),
        unvetted: None,
        guards: Vec::new(),
        unproxied: false,
    })
    .text();
    assert!(said.contains("this machine only, on port 9000"), "{said}");
    assert!(said.contains("under Operators"), "{said}");
    assert!(
        !said.contains("as komga"),
        "a loopback service was given a name: {said}"
    );
}

/// A proxied service is reached by name through the stack's proxy, and on this machine
/// alone where the stack runs none.
#[test]
fn a_proxied_service_is_shown_by_name_through_the_proxy_or_on_this_machine_alone() {
    let proxied = Reached::Proxied {
        port: 2586,
        hostname: "push".to_owned(),
        group: None,
    };
    let fronted = installs(&listing(vec![recorded("ntfy", Some(proxied.clone()))])).text();
    let alone = installs(&Installs {
        unproxied: true,
        ..listing(vec![recorded("ntfy", Some(proxied))])
    })
    .text();

    assert!(
        fronted.contains(
            "this machine on port 2586, and the household as push through the stack's proxy"
        ),
        "{fronted}"
    );
    assert!(
        alone.contains("this machine only, on port 2586, since the stack runs no proxy"),
        "{alone}"
    );
    assert!(!alone.contains("as push"), "{alone}");
}

#[test]
fn a_service_the_manifest_named_no_group_for_is_shown_without_one() {
    let said = installs(&Installs {
        demand: None,
        nonconforming: Vec::new(),
        proof: None,
        rehearsed: false,
        agreement: None,
        removal: None,
        installed: vec![recorded(
            "komga",
            Some(Reached::Loopback {
                port: 9000,
                group: None,
            }),
        )],
        install: None,
        update: None,
        substituted: Vec::new(),
        sources: Vec::new(),
        unvetted: None,
        guards: Vec::new(),
        unproxied: false,
    })
    .text();
    assert!(said.contains("this machine only, on port 9000"), "{said}");
    assert!(!said.contains("under"), "{said}");
}
