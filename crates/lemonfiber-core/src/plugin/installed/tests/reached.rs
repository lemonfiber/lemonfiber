use lemonfiber_plugin::Bind;

use super::{installed, placed, set, whole};
use crate::plugin::Reached;

/// The tier is each service's own, read from the field that decides it rather
/// than from anything held beside the plugin.
#[test]
fn each_service_keeps_the_tier_its_own_declaration_names() {
    let record = whole();
    assert_eq!(
        placed(record.as_ref(), "komga").map(|one| one.reached),
        Some(Some(Reached::Household {
            port: 25600,
            hostname: "comics".to_owned(),
            group: Some("Library".to_owned()),
        }))
    );
    assert_eq!(
        placed(record.as_ref(), "komga-sidecar").map(|one| one.reached),
        Some(Some(Reached::Loopback {
            port: 9000,
            group: Some("Library".to_owned()),
        }))
    );
}

/// The correction that mattered: only the proxy is the wider tier's alone. An
/// operator surface still appears on the bundled dashboard, so the group and the
/// tier its link is rendered from both have to survive the install.
#[test]
fn a_loopback_service_keeps_the_dashboard_group_and_the_tier_its_link_is_built_from() {
    let one = placed(whole().as_ref(), "komga-sidecar").and_then(|one| one.reached);
    assert_eq!(one.as_ref().map(Reached::port), Some(9000));
    assert_eq!(one.as_ref().map(Reached::group), Some(Some("Library")));
    assert_eq!(one.as_ref().map(Reached::hostname), Some(None));
}

/// The three readings are the facts the arms carry, so a caller need not write
/// the match itself and get one arm of it wrong.
#[test]
fn what_a_household_service_is_reached_by_reads_the_same_either_way() {
    let one = placed(whole().as_ref(), "komga").and_then(|one| one.reached);
    assert_eq!(one.as_ref().map(Reached::port), Some(25600));
    assert_eq!(one.as_ref().map(Reached::hostname), Some(Some("comics")));
    assert_eq!(one.as_ref().map(Reached::group), Some(Some("Library")));
}

/// A label is a fact about one container. Defaulting to the plugin's id would
/// give two services of one plugin the same address.
#[test]
fn a_service_the_manifest_names_no_label_for_answers_on_its_own_id() {
    let record = installed(|manifest| manifest.wirings.clear());
    assert_eq!(
        placed(record.as_ref(), "komga").map(|one| one.reached),
        Some(Some(Reached::Household {
            port: 25600,
            hostname: "komga".to_owned(),
            group: None,
        }))
    );
}

/// The tier decides, and the record cannot say otherwise: a loopback service has
/// nowhere to put a hostname, whatever the manifest asked for.
#[test]
fn a_loopback_service_carries_no_hostname_even_where_one_is_declared() {
    let record = installed(|manifest| set(manifest, |service| service.bind = Some(Bind::Loopback)));
    assert_eq!(
        placed(record.as_ref(), "komga").map(|one| one.reached),
        Some(Some(Reached::Loopback {
            port: 25600,
            group: Some("Library".to_owned()),
        }))
    );
}

/// A proxied service keeps the label the household reaches it at through the proxy.
#[test]
fn a_proxied_service_carries_its_hostname() {
    let record = installed(|manifest| set(manifest, |service| service.bind = Some(Bind::Proxied)));
    assert_eq!(
        placed(record.as_ref(), "komga").map(|one| one.reached),
        Some(Some(Reached::Proxied {
            port: 25600,
            hostname: "comics".to_owned(),
            group: Some("Library".to_owned()),
        }))
    );
}

#[test]
fn a_service_with_no_listener_is_recorded_as_reached_by_nothing() {
    let record = installed(|manifest| {
        set(manifest, |service| {
            service.port = None;
            service.bind = None;
        });
    });
    assert_eq!(
        placed(record.as_ref(), "komga").map(|one| one.reached),
        Some(None)
    );
}

/// A port with no tier is a manifest the reader refuses. Were one to reach here
/// the record says it is reached by nothing rather than choosing a tier for it —
/// the one answer that cannot put an admin surface on the household network.
#[test]
fn a_port_with_no_tier_is_recorded_as_reached_by_nothing_rather_than_placed() {
    let record = installed(|manifest| set(manifest, |service| service.bind = None));
    assert_eq!(
        placed(record.as_ref(), "komga").map(|one| one.reached),
        Some(None)
    );
}
