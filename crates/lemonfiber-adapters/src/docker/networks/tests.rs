use std::collections::HashMap;

use bollard::models::{Ipam, IpamConfig};
use lemonfiber_ports::docker::{Network, Shape, Wanted};

use super::{read, request, MASQUERADE, NETWORK_LABEL, NO_MASQUERADE, PROJECT_LABEL};

fn shape(internal: bool, masquerade: bool) -> Shape {
    Shape {
        subnet: Some("10.80.0.8/29".to_owned()),
        ip_range: Some("10.80.0.12/30".to_owned()),
        internal,
        masquerade,
    }
}

fn listed(labels: &[(&str, &str)], options: &[(&str, &str)]) -> bollard::models::Network {
    bollard::models::Network {
        name: Some("lemonfiber_media".to_owned()),
        id: Some("abc123".to_owned()),
        internal: Some(true),
        labels: Some(
            labels
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
        ),
        options: Some(
            options
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
        ),
        ipam: Some(Ipam {
            config: Some(vec![IpamConfig {
                subnet: Some("10.80.0.8/29".to_owned()),
                ip_range: Some("10.80.0.12/30".to_owned()),
                gateway: Some("10.80.0.9".to_owned()),
                ..IpamConfig::default()
            }]),
            ..Ipam::default()
        }),
        ..bollard::models::Network::default()
    }
}

#[test]
fn a_network_is_created_bridged_labelled_for_its_project_with_its_shape() {
    let wanted = shape(false, false);
    let asked = request(Wanted {
        name: "lemonfiber_media",
        project: "lemonfiber",
        declared: "media",
        shape: &wanted,
    });
    assert_eq!(asked.name, "lemonfiber_media");
    assert_eq!(asked.driver.as_deref(), Some("bridge"));
    assert_eq!(asked.internal, Some(false));
    assert_eq!(
        asked.labels,
        Some(HashMap::from([
            (PROJECT_LABEL.to_owned(), "lemonfiber".to_owned()),
            (NETWORK_LABEL.to_owned(), "media".to_owned()),
        ]))
    );
    assert_eq!(
        asked.options,
        Some(HashMap::from([(
            MASQUERADE.to_owned(),
            NO_MASQUERADE.to_owned()
        )]))
    );
    let pool = asked
        .ipam
        .and_then(|ipam| ipam.config)
        .and_then(|config| config.into_iter().next())
        .unwrap_or_default();
    assert_eq!(pool.subnet.as_deref(), Some("10.80.0.8/29"));
    assert_eq!(pool.ip_range.as_deref(), Some("10.80.0.12/30"));
    assert_eq!(pool.gateway, None);
}

#[test]
fn a_network_translating_addresses_is_created_with_no_option() {
    let wanted = shape(true, true);
    let asked = request(Wanted {
        name: "lemonfiber_media",
        project: "lemonfiber",
        declared: "media",
        shape: &wanted,
    });
    assert_eq!(asked.internal, Some(true));
    assert_eq!(asked.options, None);
}

#[test]
fn a_listed_network_reads_back_as_the_shape_it_was_created_with() {
    let read = read(listed(
        &[(PROJECT_LABEL, "lemonfiber"), (NETWORK_LABEL, "media")],
        &[(MASQUERADE, NO_MASQUERADE)],
    ));
    assert_eq!(
        read,
        Some(Network {
            name: "lemonfiber_media".to_owned(),
            declared: "media".to_owned(),
            id: "abc123".to_owned(),
            shape: shape(true, false),
            gateway: Some("10.80.0.9".to_owned()),
        })
    );
}

#[test]
fn a_network_without_its_declared_name_is_not_one_of_the_project() {
    assert_eq!(read(listed(&[(PROJECT_LABEL, "lemonfiber")], &[])), None);
    let masquerading = read(listed(&[(NETWORK_LABEL, "media")], &[]));
    assert_eq!(
        masquerading.map(|network| network.shape.masquerade),
        Some(true)
    );
}
