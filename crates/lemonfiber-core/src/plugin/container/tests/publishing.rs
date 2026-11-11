use super::super::super::installed::{Placed, Reached};
use super::{document, list, placed, HOUSEHOLD, LOOPBACK};

/// An operator surface is published where the stack publishes its own.
///
/// The address is this build's answer to the tier rather than anything a manifest
/// said: the only thing changed here is the tier.
#[test]
fn each_tier_is_published_on_the_interface_the_tier_decides() {
    assert!(document(placed()).contains(&format!("{HOUSEHOLD}:25600:25600")));

    let operator = document(Placed {
        reached: Some(Reached::Loopback {
            port: 25600,
            group: None,
        }),
        ..placed()
    });
    assert!(
        operator.contains(&format!("{LOOPBACK}:25600:25600")),
        "got: {operator}"
    );
    assert!(!operator.contains(HOUSEHOLD), "got: {operator}");
}

/// A proxied service and every further port it publishes are on this machine's loopback
/// alone: the household reaches it by name through the proxy, never at a port.
#[test]
fn a_proxied_service_publishes_on_loopback_alone() {
    let mut proxied = Placed {
        reached: Some(Reached::Proxied {
            port: 25600,
            hostname: "comics".to_owned(),
            group: None,
        }),
        ..placed()
    };
    proxied.fields.publish = vec![lemonfiber_manifest::container::Publish {
        port: 25601,
        listens: None,
    }];

    let entry = document(proxied);

    assert!(
        entry.contains(&format!("{LOOPBACK}:25600:25600")),
        "got: {entry}"
    );
    assert!(
        entry.contains(&format!("{LOOPBACK}:25601:25601")),
        "got: {entry}"
    );
    assert!(!entry.contains(HOUSEHOLD), "got: {entry}");
}

/// A service with no listener is written with no ports at all.
///
/// Not a port mapping onto nothing, and not an empty list: a service that
/// publishes nothing is a different entry rather than the same one with a blank
/// in it.
#[test]
fn a_service_with_no_listener_publishes_nothing() {
    let entry = document(Placed {
        reached: None,
        ..placed()
    });
    assert!(!entry.contains("ports:"), "got: {entry}");
    assert!(entry.contains("volumes:"), "got: {entry}");
}

/// The port a service speaks its contracts on is published on this machine's loopback
/// alone, at whatever host port the engine gives it; one that speaks nothing publishes
/// the port it listens on no further than the stack's network.
#[test]
fn the_port_an_adapter_speaks_on_is_published_on_loopback_at_a_port_the_engine_picks() {
    let adapter = document(Placed {
        reached: None,
        listens: Some(8080),
        speaks: vec!["media.serve@1".to_owned()],
        fronts: None,
        upstream: std::collections::BTreeMap::new(),
        ..placed()
    });
    assert!(
        adapter.contains(&format!("{LOOPBACK}::8080")),
        "got: {adapter}"
    );
    let silent = document(Placed {
        reached: None,
        listens: Some(8080),
        ..placed()
    });
    assert!(!silent.contains("ports:"), "got: {silent}");
}

/// A service that speaks a contract resolves the name containers reach this machine by to
/// the engine's host gateway, so it can tell the core what happened; one that speaks none
/// resolves nothing extra.
#[test]
fn an_adapter_reaches_this_machine_by_the_name_containers_do() {
    let adapter = document(Placed {
        speaks: vec!["library.curate@1".to_owned()],
        listens: Some(8080),
        ..placed()
    });
    let hosts = list(&adapter, "komga", "extra_hosts");

    assert_eq!(
        hosts,
        ["host.docker.internal:host-gateway"],
        "got: {adapter}"
    );
    assert!(!document(placed()).contains("extra_hosts"));
}
