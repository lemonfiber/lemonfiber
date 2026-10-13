use std::collections::BTreeMap;

use lemonfiber_manifest::container::{Network, Publish, UrlOf, Value};
use lemonfiber_manifest::{Manifest, Wiring};
use serde_norway::Value as Yaml;

use super::{generated, Project, Reading};
use crate::plugin::{Installed, Placed};
use crate::test_support::{a_contracted_media_server, a_placed, an_installed, MEDIA_UPSTREAM};
use crate::topology::{Placed as Where, PROXY_ON_PROXY};
use crate::wiring::Chosen;

fn carried() -> Manifest {
    let Ok(manifest) = Manifest::from_toml(lemonfiber_bundled::STACK) else {
        unreachable!("the embedded stack parses");
    };
    manifest
}

fn project(manifest: &Manifest, installed: &[Installed]) -> Project {
    generated(manifest, installed, &Chosen::read(None), &Reading::bare())
}

fn entry(project: &Project, id: &str) -> Yaml {
    let document: Yaml = serde_norway::from_str(&project.text).unwrap_or_default();
    document
        .get("services")
        .and_then(|services| services.get(id))
        .cloned()
        .unwrap_or_default()
}

fn text(entry: &Yaml, key: &str) -> String {
    entry
        .get(key)
        .and_then(Yaml::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn environment(entry: &Yaml, key: &str) -> String {
    entry
        .get("environment")
        .and_then(|environment| environment.get(key))
        .and_then(Yaml::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn ports(entry: &Yaml) -> Vec<String> {
    entry
        .get("ports")
        .and_then(Yaml::as_sequence)
        .map(|ports| {
            ports
                .iter()
                .filter_map(|port| port.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn asking(service: &str, key: &str, url: UrlOf) -> Placed {
    let mut placed = a_placed(service, &[], None, None);
    placed.fields.env = BTreeMap::from([(key.to_owned(), Value::Url(url))]);
    placed
}

#[test]
fn an_adapter_and_its_upstream_are_both_written_and_share_a_network_of_their_own() {
    let project = project(&carried(), &[a_contracted_media_server(Some("jellyfin"))]);
    for id in ["media-adapter", MEDIA_UPSTREAM] {
        assert_ne!(entry(&project, id), Yaml::Null, "{id} is written");
    }
    let Some(Where::On(adapter)) = project.topology.placed.get("media-adapter") else {
        unreachable!("the adapter is on networks of its own");
    };
    let Some(Where::On(upstream)) = project.topology.placed.get(MEDIA_UPSTREAM) else {
        unreachable!("the upstream is on networks of its own");
    };
    assert!(adapter.keys().any(|network| upstream.contains_key(network)));
}

#[test]
fn an_address_narrowed_to_a_medium_is_the_one_claimant_filing_it() {
    let reader = asking(
        "reader",
        "CURATOR_URL",
        UrlOf {
            url_of: "library.curate".into(),
            files: Some("tv".into()),
            native: None,
        },
    );
    let project = project(&carried(), &[an_installed("reader", vec![reader])]);
    assert_eq!(
        environment(&entry(&project, "reader"), "CURATOR_URL"),
        "http://sonarr:8989"
    );
}

#[test]
fn an_address_in_the_upstreams_own_api_reaches_the_upstream_an_adapter_fronts() {
    let reader = asking(
        "reader",
        "SERVER_URL",
        UrlOf {
            url_of: "media.serve".into(),
            files: None,
            native: Some("jellyfin".into()),
        },
    );
    let mut manifest = carried();
    manifest
        .services
        .retain(|service| !service.provides.iter().any(|one| one == "media.serve"));
    let project = project(
        &manifest,
        &[
            a_contracted_media_server(Some("jellyfin")),
            an_installed("reader", vec![reader]),
        ],
    );
    assert_eq!(
        environment(&entry(&project, "reader"), "SERVER_URL"),
        format!("http://{MEDIA_UPSTREAM}:9000")
    );
}

#[test]
fn a_service_inside_the_egress_guard_has_no_network_and_its_ports_are_the_guards() {
    let mut inside = a_placed("client", &[], None, Some(18999));
    inside.fields.network = Some(Network {
        via: "network.egress-guard".into(),
    });
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
}

#[test]
fn a_stack_image_without_a_digest_is_written_by_its_tag() {
    let mut manifest = carried();
    let Some(sonarr) = manifest.services.iter_mut().find(|one| one.id == "sonarr") else {
        unreachable!("the stack runs sonarr");
    };
    sonarr.digest = None;
    let tag = sonarr.tag.clone();
    let project = project(&manifest, &[]);
    assert_eq!(
        text(&entry(&project, "sonarr"), "image"),
        format!("lscr.io/linuxserver/sonarr:{tag}")
    );
}

#[test]
fn the_guard_and_the_proxy_hold_their_fixed_addresses_on_fixed_subnets() {
    let mut manifest = carried();
    for (id, capability) in [("door", "stream.guard"), ("caddy", "proxy.front")] {
        if let Some(service) = manifest.services.iter_mut().find(|one| one.id == id) {
            service.provides.push(capability.into());
        }
    }
    for capability in ["proxy.front", "media.serve"] {
        manifest.wirings.push(Wiring {
            by: "door".into(),
            asks: Some(capability.into()),
            each: false,
            filled_by: None,
            to: None,
            why: None,
        });
    }
    let project = project(&manifest, &[]);
    let document: Yaml = serde_norway::from_str(&project.text).unwrap_or_default();
    let declared = document.get("networks").cloned().unwrap_or_default();
    assert!(project.text.contains(&PROXY_ON_PROXY.to_string()));
    assert!(declared
        .as_mapping()
        .is_some_and(|networks| networks.values().any(|one| one.get("ipam").is_some())));
    let caddy = entry(&project, "caddy");
    assert!(caddy
        .get("networks")
        .and_then(Yaml::as_mapping)
        .is_some_and(|networks| networks
            .values()
            .any(|one| one.get("ipv4_address").is_some())));
}

fn with_url(manifest: &mut Manifest, service: &str, key: &str, url: UrlOf) {
    if let Some(found) = manifest.services.iter_mut().find(|one| one.id == service) {
        found.env.insert(key.to_owned(), Value::Url(url));
    }
}

fn url(url_of: &str, files: Option<&str>, native: Option<&str>) -> UrlOf {
    UrlOf {
        url_of: url_of.into(),
        files: files.map(Into::into),
        native: native.map(Into::into),
    }
}

#[test]
fn an_address_for_what_the_service_asks_is_the_one_service_the_ask_reaches() {
    let mut manifest = carried();
    with_url(
        &mut manifest,
        "prowlarr",
        "SOLVER_URL",
        url("indexer.proxy", None, None),
    );
    let project = project(&manifest, &[]);
    assert_eq!(
        environment(&entry(&project, "prowlarr"), "SOLVER_URL"),
        "http://flaresolverr:8191"
    );
}

#[test]
fn an_address_an_ask_reaches_several_services_for_is_written_for_none() {
    let mut manifest = carried();
    with_url(
        &mut manifest,
        "prowlarr",
        "CURATOR_URL",
        url("library.curate", None, None),
    );
    let project = project(&manifest, &[]);
    assert_eq!(environment(&entry(&project, "prowlarr"), "CURATOR_URL"), "");
}

#[test]
fn an_address_of_a_service_inside_the_guard_is_the_guards() {
    let reader = asking(
        "reader",
        "TORRENT_URL",
        url("download.torrent", None, Some("qbittorrent")),
    );
    let project = project(&carried(), &[an_installed("reader", vec![reader])]);
    assert_eq!(
        environment(&entry(&project, "reader"), "TORRENT_URL"),
        "http://gluetun:8081"
    );
}

#[test]
fn an_address_narrowed_to_a_medium_never_reaches_the_asker_itself() {
    let mut reader = asking(
        "reader",
        "CURATOR_URL",
        url("library.curate", Some("tv"), None),
    );
    reader.provides = vec!["library.curate".into()];
    reader.media_types = vec!["tv".into()];
    let project = project(&carried(), &[an_installed("reader", vec![reader])]);
    let Some(Where::On(on)) = project.topology.placed.get("reader") else {
        unreachable!("the reader is on networks of its own");
    };
    assert!(!on
        .keys()
        .any(|network| network.contains("reader__library.curate__reader")));
}

#[test]
fn an_adapter_filing_a_medium_is_reached_itself_where_the_ask_is_not_in_its_upstreams_api() {
    let mut server = a_contracted_media_server(None);
    for placed in &mut server.services {
        placed.media_types = vec!["movies".into()];
    }
    let reader = asking(
        "reader",
        "SERVER_URL",
        url("media.serve", Some("movies"), None),
    );
    let mut manifest = carried();
    manifest
        .services
        .retain(|service| !service.provides.iter().any(|one| one == "media.serve"));
    let project = project(&manifest, &[server, an_installed("reader", vec![reader])]);
    assert_eq!(
        environment(&entry(&project, "reader"), "SERVER_URL"),
        "http://media-adapter:8080"
    );
}

#[test]
fn a_chosen_filler_is_the_one_a_service_runs_inside() {
    let mut inside = a_placed("client", &[], None, None);
    inside.fields.network = Some(Network {
        via: "network.egress-guard".into(),
    });
    let chosen = Chosen::read(Some("network.egress-guard=gluetun"));
    let project = generated(
        &carried(),
        &[an_installed("client", vec![inside])],
        &chosen,
        &Reading::bare(),
    );
    assert_eq!(
        project.topology.placed.get("client"),
        Some(&Where::Inside("gluetun".into()))
    );
}

#[test]
fn an_adapter_inside_the_guard_has_its_contract_port_published_by_the_guard() {
    let mut adapter = a_placed("bridge", &[], None, Some(18997));
    adapter.reached = None;
    adapter.speaks = vec!["media.serve@1".into()];
    adapter.fields.network = Some(Network {
        via: "network.egress-guard".into(),
    });
    let project = project(&carried(), &[an_installed("bridge", vec![adapter])]);
    assert_eq!(
        text(&entry(&project, "bridge"), "network_mode"),
        "service:gluetun"
    );
    assert!(ports(&entry(&project, "gluetun"))
        .iter()
        .any(|written| written.ends_with(":18997")));
}

#[test]
fn an_installed_guard_the_operator_chose_holds_the_services_inside_it() {
    let guard = a_placed("tunnel", &["network.egress-guard"], None, Some(18996));
    let mut inside = a_placed("client", &[], None, Some(18995));
    inside.fields.network = Some(Network {
        via: "network.egress-guard".into(),
    });
    let project = generated(
        &carried(),
        &[
            an_installed("tunnel", vec![guard]),
            an_installed("client", vec![inside]),
        ],
        &Chosen::read(Some("network.egress-guard=tunnel")),
        &Reading::bare(),
    );
    assert_eq!(
        text(&entry(&project, "client"), "network_mode"),
        "service:tunnel"
    );
    assert!(ports(&entry(&project, "tunnel"))
        .iter()
        .any(|written| written.contains("18995")));
}
