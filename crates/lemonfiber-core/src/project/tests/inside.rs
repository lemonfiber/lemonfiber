use lemonfiber_manifest::container::Publish;
use serde_norway::Value as Yaml;

use super::{carried, entry, inside_the_guard, ports, project, sequence, text};
use crate::test_support::{a_placed, an_installed};

#[test]
fn a_service_inside_the_egress_guard_activates_the_guards_profile_and_no_other() {
    let mut inside = a_placed("client", &[], None);
    inside.fields.network = Some(inside_the_guard());
    let manifest = carried();
    let installed = [an_installed("client", vec![inside])];
    let topology = project(&manifest, &installed).topology;
    let guard: std::collections::BTreeSet<String> = manifest
        .services
        .iter()
        .filter(|service| service.id == "gluetun")
        .map(|service| service.profile.clone())
        .collect();

    let hosting = |service: &str| {
        super::super::hosting(&manifest, &installed, &topology, &[service.to_owned()])
    };

    assert!(!guard.is_empty());
    assert_eq!(hosting("client"), guard);
    assert!(hosting("gluetun").is_empty());
}

#[test]
fn a_service_inside_a_plugins_egress_guard_activates_that_plugins_profile() {
    let mut inside = a_placed("client", &[], None);
    inside.fields.network = Some(inside_the_guard());
    let mut manifest = carried();
    manifest.services.retain(|service| {
        !service
            .provides
            .iter()
            .any(|one| one == "network.egress-guard")
    });
    let installed = [
        an_installed(
            "tunnel",
            vec![a_placed("tunnel", &["network.egress-guard"], None)],
        ),
        an_installed("client", vec![inside]),
    ];
    let topology = project(&manifest, &installed).topology;

    let hosting = super::super::hosting(&manifest, &installed, &topology, &["client".to_owned()]);

    assert_eq!(
        hosting,
        std::iter::once(crate::plugin::profile("tunnel")).collect()
    );
}

#[test]
fn a_service_inside_the_egress_guard_has_no_network_and_its_ports_are_the_guards() {
    let mut inside = a_placed("client", &[], Some(18999));
    inside.fields.network = Some(inside_the_guard());
    inside.fields.publish = vec![Publish {
        port: 18998,
        listens: None,
    }];
    let project = project(&carried(), &[an_installed("client", vec![inside])]);
    let client = entry(&project, "client");
    assert_eq!(text(&client, "network_mode"), "service:gluetun");
    assert!(ports(&client).is_empty());
    let guard = ports(&entry(&project, "gluetun"));
    for port in ["18999", "18998"] {
        assert!(guard.iter().any(|written| written.contains(port)), "{port}");
    }
    let networks = entry(&project, "gluetun")
        .get("networks")
        .and_then(Yaml::as_mapping)
        .cloned()
        .unwrap_or_default();
    assert!(!networks.is_empty());
    for (name, held) in &networks {
        let aliases = sequence(held, "aliases");
        assert!(
            aliases.iter().any(|alias| alias == "client")
                && !aliases.iter().any(|alias| alias == "gluetun"),
            "{name:?}: {aliases:?}"
        );
    }
}
