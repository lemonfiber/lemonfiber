use std::collections::BTreeMap;

use lemonfiber_manifest::container::{Network, Publish, UrlOf, Value};
use lemonfiber_manifest::Manifest;
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
    let mut placed = a_placed(service, &[], None);
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
fn a_service_inside_the_egress_guard_activates_the_guards_profile_and_no_other() {
    let mut inside = a_placed("client", &[], None);
    inside.fields.network = Some(Network {
        via: "network.egress-guard".into(),
    });
    let manifest = carried();
    let installed = [an_installed("client", vec![inside])];
    let topology = project(&manifest, &installed).topology;
    let guard: std::collections::BTreeSet<String> = manifest
        .services
        .iter()
        .filter(|service| service.id == "gluetun")
        .map(|service| service.profile.clone())
        .collect();

    let hosting =
        |service: &str| super::hosting(&manifest, &installed, &topology, &[service.to_owned()]);

    assert!(!guard.is_empty());
    assert_eq!(hosting("client"), guard);
    assert!(hosting("gluetun").is_empty());
}

#[test]
fn a_service_inside_a_plugins_egress_guard_activates_that_plugins_profile() {
    let mut inside = a_placed("client", &[], None);
    inside.fields.network = Some(Network {
        via: "network.egress-guard".into(),
    });
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

    let hosting = super::hosting(&manifest, &installed, &topology, &["client".to_owned()]);

    assert_eq!(
        hosting,
        std::iter::once(crate::plugin::profile("tunnel")).collect()
    );
}

#[test]
fn a_service_inside_the_egress_guard_has_no_network_and_its_ports_are_the_guards() {
    let mut inside = a_placed("client", &[], Some(18999));
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
fn a_confined_service_nothing_reaches_and_that_publishes_nothing_is_on_no_network() {
    let mut loner = a_placed("loner", &[], None);
    loner.fields.posture = lemonfiber_manifest::container::Posture::Confined;
    let project = project(&carried(), &[an_installed("loner", vec![loner])]);
    assert_eq!(project.topology.placed.get("loner"), Some(&Where::Nowhere));
    assert_eq!(text(&entry(&project, "loner"), "network_mode"), "none");
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
    let project = project(&carried(), &[]);
    let document: Yaml = serde_norway::from_str(&project.text).unwrap_or_default();
    let declared = document.get("networks").cloned().unwrap_or_default();
    assert!(project.text.contains(&PROXY_ON_PROXY.to_string()));
    assert!(declared.as_mapping().is_some_and(|networks| {
        networks.values().any(|one| {
            one.get("ipam")
                .and_then(|ipam| ipam.get("config")?.get(0)?.get("ip_range"))
                .is_some()
        })
    }));
    let caddy = entry(&project, "caddy");
    assert!(caddy
        .get("networks")
        .and_then(Yaml::as_mapping)
        .is_some_and(|networks| networks
            .values()
            .any(|one| one.get("ipv4_address").is_some())));
}

/// Whether any network the project declares holds fixed addresses.
fn fixes_addresses(project: &Project) -> bool {
    let document: Yaml = serde_norway::from_str(&project.text).unwrap_or_default();
    document
        .get("networks")
        .and_then(Yaml::as_mapping)
        .is_some_and(|networks| {
            networks.values().any(|one| {
                one.get("ipam")
                    .and_then(|ipam| ipam.get("config")?.get(0)?.get("ip_range"))
                    .is_some()
            })
        })
}

#[test]
fn a_stack_no_one_service_guards_fixes_no_address() {
    let mut nothing = carried();
    for service in &mut nothing.services {
        service.provides.retain(|named| named != "stream.guard");
    }
    let mut two = carried();
    if let Some(caddy) = two.services.iter_mut().find(|one| one.id == "caddy") {
        caddy.provides.push("stream.guard".into());
    }
    assert!(fixes_addresses(&project(&carried(), &[])));
    assert!(!fixes_addresses(&project(&nothing, &[])));
    assert!(!fixes_addresses(&project(&two, &[])));
}

/// A plugin's service asking for `capability` and given its address as `key`.
fn asking_for(service: &str, key: &str, capability: &str) -> Placed {
    let mut placed = asking(service, key, url(capability, None, None));
    placed.asks = vec![crate::plugin::Asking {
        capability: capability.to_owned(),
        each: false,
        native: None,
    }];
    placed
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
    let reader = asking_for("reader", "SOLVER_URL", "indexer.proxy");
    let project = project(&carried(), &[an_installed("reader", vec![reader])]);
    assert_eq!(
        environment(&entry(&project, "reader"), "SOLVER_URL"),
        "http://flaresolverr:8191"
    );
}

#[test]
fn an_address_an_ask_reaches_several_services_for_is_written_for_none() {
    let mut reader = asking_for("reader", "CURATOR_URL", "library.curate");
    for ask in &mut reader.asks {
        ask.each = true;
    }
    let project = project(&carried(), &[an_installed("reader", vec![reader])]);
    assert_eq!(environment(&entry(&project, "reader"), "CURATOR_URL"), "");
    assert!(
        project.topology.placed.contains_key("reader"),
        "the reader is written"
    );
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
    let mut inside = a_placed("client", &[], None);
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
    let mut adapter = a_placed("bridge", &[], Some(18997));
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
        .any(|written| *written == format!("{}::18997", super::entry::LOOPBACK)));
}

/// The core reaches an adapter at its contract port published on the loopback interface,
/// at a host port the engine chooses, and the adapter publishes nothing beyond it.
#[test]
fn an_adapters_contract_port_is_published_on_loopback_alone() {
    let project = project(&carried(), &[a_contracted_media_server(Some("jellyfin"))]);
    let published = ports(&entry(&project, "media-adapter"));
    let loopback = super::entry::LOOPBACK;
    assert!(
        published.contains(&format!("{loopback}::8080")),
        "{published:?}"
    );
    assert!(
        published
            .iter()
            .all(|written| written.starts_with(&format!("{loopback}:"))),
        "{published:?}"
    );
}

#[test]
fn an_installed_guard_the_operator_chose_holds_the_services_inside_it() {
    let guard = a_placed("tunnel", &["network.egress-guard"], Some(18996));
    let mut inside = a_placed("client", &[], Some(18995));
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

fn solver_installed(reached: Option<u16>) -> Project {
    let mut manifest = carried();
    manifest
        .services
        .retain(|service| service.id != "flaresolverr");
    let reader = asking_for("reader", "SOLVER_URL", "indexer.proxy");
    let mut solver = a_placed("solver", &["indexer.proxy"], reached);
    solver.listens = None;
    project(
        &manifest,
        &[
            an_installed("solver", vec![solver]),
            an_installed("reader", vec![reader]),
        ],
    )
}

#[test]
fn an_address_of_a_service_with_no_port_of_its_own_inside_is_the_one_it_is_reached_on() {
    assert_eq!(
        environment(
            &entry(&solver_installed(Some(18994)), "reader"),
            "SOLVER_URL"
        ),
        "http://solver:18994"
    );
}

#[test]
fn an_address_of_a_service_that_answers_on_no_port_is_written_for_none() {
    assert_eq!(
        environment(&entry(&solver_installed(None), "reader"), "SOLVER_URL"),
        ""
    );
}

#[test]
fn on_an_nfs_export_every_service_taking_data_mounts_the_one_declared_volume() {
    let export = crate::config::storage::Export {
        server: "192.168.1.20".to_owned(),
        path: "/export/$media".to_owned(),
    };
    let reading = Reading {
        nfs: Some(&export),
        ..Reading::bare()
    };
    let manifest = carried();
    let mounted = generated(&manifest, &[], &Chosen::read(None), &reading);
    let document: Yaml = serde_norway::from_str(&mounted.text).unwrap_or_default();
    let volume = document
        .get("volumes")
        .and_then(|volumes| volumes.get("data"))
        .cloned()
        .unwrap_or_default();
    assert_eq!(text(&volume, "driver"), "local");
    let options = volume.get("driver_opts").cloned().unwrap_or_default();
    assert_eq!(text(&options, "type"), "nfs");
    assert_eq!(
        text(&options, "o"),
        "addr=192.168.1.20,nfsvers=4,rw,hard,timeo=600,retrans=2"
    );
    assert_eq!(text(&options, "device"), ":/export/$$media");
    let taking: Vec<&str> = manifest
        .services
        .iter()
        .filter(|service| service.takes_data && !service.host_managed)
        .map(|service| service.id.as_str())
        .collect();
    assert!(
        !taking.is_empty(),
        "the carried stack has services that take data"
    );
    for id in taking {
        let volumes = entry(&mounted, id)
            .get("volumes")
            .and_then(Yaml::as_sequence)
            .cloned()
            .unwrap_or_default();
        let library: Vec<&str> = volumes
            .iter()
            .filter_map(Yaml::as_str)
            .filter(|volume| volume.ends_with(":/data"))
            .collect();
        assert_eq!(library, ["data:/data"], "{id}");
    }
    let bound = project(&manifest, &[]);
    assert!(!bound.text.contains("type: nfs"), "{}", bound.text);
}

#[test]
fn a_port_standing_beside_moved_is_published_where_it_was_moved_to() {
    let manifest = carried();
    let Some(service) = manifest
        .services
        .iter()
        .find(|service| service.id == "sonarr" && service.port.is_some())
    else {
        unreachable!("the carried stack publishes sonarr");
    };
    let from = service.port.unwrap_or_default();
    let moved = [crate::model::MovedReport {
        service: "sonarr".to_owned(),
        from,
        to: 18989,
    }];
    let reading = Reading {
        moved: &moved,
        ..Reading::bare()
    };
    let beside = generated(&manifest, &[], &Chosen::read(None), &reading);
    let published = ports(&entry(&beside, "sonarr"));
    let inside = service.listens.unwrap_or(from);
    assert!(
        published
            .iter()
            .any(|port| port.ends_with(&format!(":18989:{inside}"))),
        "{published:?}"
    );
    assert!(
        !published
            .iter()
            .any(|port| port.ends_with(&format!(":{from}:{inside}"))),
        "{published:?}"
    );
    let radarr = ports(&entry(&beside, "radarr"));
    assert_eq!(radarr, ports(&entry(&project(&manifest, &[]), "radarr")));
}

/// A stack service a first-party plugin's service has moved in place of is left out of
/// the project, and the plugin's service is written under its id.
#[test]
fn a_stack_service_moved_in_place_is_left_out_and_its_stand_in_written() {
    let mut moved = a_placed("jellyfin", &["media.serve"], Some(8096));
    moved.replaces = Some("jellyfin".to_owned());
    moved.image = "ghcr.io/lemonfiber/plugin-jellyfin".to_owned();
    let project = project(
        &carried(),
        &[crate::test_support::pinned_as(an_installed(
            "jellyfin",
            vec![moved],
        ))],
    );
    assert!(
        text(&entry(&project, "jellyfin"), "image")
            .starts_with("ghcr.io/lemonfiber/plugin-jellyfin"),
        "{}",
        project.text
    );
    assert_eq!(project.text.matches("\n  jellyfin:\n").count(), 1);
    let configuration = |project: &Project| {
        sequence(&entry(project, "jellyfin"), "volumes")
            .into_iter()
            .find_map(|volume| {
                volume
                    .split_once(':')
                    .map(|(from, _)| from.to_owned())
                    .filter(|from| from.starts_with("./config/"))
            })
    };
    let standing = project_of_the_stack_alone();
    assert_eq!(
        configuration(&project),
        Some("./config/jellyfin".to_owned())
    );
    assert_eq!(configuration(&project), configuration(&standing));
}

/// Every service of a plugin the bundle installs is written into the stack profile its
/// bundle entry names, so the forms holding that profile start and stop it,
/// and a third-party plugin's into a profile of its own.
#[test]
fn a_plugin_the_bundle_installs_is_written_into_the_profile_its_entry_names() {
    let moved: Vec<Installed> = crate::test_support::media::moved().into_iter().collect();
    let stranger = an_installed(
        "comics",
        vec![a_placed("comics", &["library.read"], Some(25600))],
    );
    let written = project(&carried(), &[moved.clone(), vec![stranger]].concat());
    let profile: Vec<String> = moved.iter().map(crate::plugin::profile_of).collect();

    for id in [
        crate::test_support::media::PLUGIN,
        crate::test_support::media::ADAPTER,
    ] {
        assert_eq!(sequence(&entry(&written, id), "profiles"), profile, "{id}");
    }
    assert_ne!(
        profile,
        vec![crate::plugin::profile(crate::test_support::media::PLUGIN)]
    );
    assert_eq!(
        sequence(&entry(&written, "comics"), "profiles"),
        vec![crate::plugin::profile("comics")]
    );
}

/// The project of the stack this build carries, with nothing installed.
fn project_of_the_stack_alone() -> Project {
    project(&carried(), &[])
}

/// Only a first-party plugin's service taking the stack's service's own id moves in place
/// of it: one a third party brings, or a first-party service under another id, naming the
/// stack's service as the one it replaces puts nothing out.
#[test]
fn only_a_first_party_service_taking_its_id_moves_in_place_of_a_stack_service() {
    let replacing = |service: &str| {
        let mut placed = a_placed(service, &["media.serve"], Some(8096));
        placed.replaces = Some("jellyfin".to_owned());
        an_installed("jellyfin", vec![placed])
    };
    let pinned = crate::test_support::pinned_as;

    assert!(super::inputs::moved_in_place(
        &[pinned(replacing("jellyfin"))],
        "jellyfin"
    ));
    assert!(!super::inputs::moved_in_place(
        &[replacing("jellyfin")],
        "jellyfin"
    ));
    assert!(!super::inputs::moved_in_place(
        &[pinned(replacing("elsewhere"))],
        "jellyfin"
    ));
}

/// A plugin service the household reaches at a hostname shares a network with the proxy
/// that routes the hostname to it, and one reached only on the loopback interface does not.
#[test]
fn a_plugin_service_the_household_reaches_shares_a_network_with_the_proxy() {
    let mut household = a_placed("shelf", &[], Some(13378));
    household.reached = Some(crate::plugin::Reached::Household {
        port: 13378,
        hostname: "shelf".to_owned(),
        group: None,
    });
    let loopback = a_placed("panel", &[], Some(13379));
    let project = project(
        &carried(),
        &[an_installed("shelf", vec![household, loopback])],
    );

    let networks = |id: &str| match project.topology.placed.get(id) {
        Some(Where::On(networks)) => networks.keys().cloned().collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    let proxy = networks("caddy");
    assert!(
        networks("shelf").iter().any(|one| proxy.contains(one)),
        "{proxy:?}"
    );
    assert!(
        !networks("panel").iter().any(|one| proxy.contains(one)),
        "{proxy:?}"
    );
}

#[test]
fn a_plugin_services_ask_joins_it_to_that_asks_network_and_no_other() {
    let mut asker = a_placed("subs", &[], None);
    asker.asks = vec![crate::plugin::Asking {
        native: None,
        capability: "media.serve".to_owned(),
        each: false,
    }];
    let project = project(&carried(), &[an_installed("subs", vec![asker])]);

    let networks = |id: &str| match project.topology.placed.get(id) {
        Some(Where::On(networks)) => networks.keys().cloned().collect::<Vec<_>>(),
        _ => Vec::new(),
    };
    assert_eq!(
        networks("subs"),
        ["ask__subs__media.serve__jellyfin", "own__subs"]
    );
    assert!(networks("jellyfin").contains(&"ask__subs__media.serve__jellyfin".to_owned()));
    let members = project
        .topology
        .placed
        .iter()
        .filter(|(_, placed)| {
            matches!(placed, Where::On(networks) if networks.contains_key("ask__subs__media.serve__jellyfin"))
        })
        .map(|(id, _)| id.clone())
        .collect::<Vec<_>>();
    assert_eq!(members, ["jellyfin", "subs"]);
}

fn sequence(entry: &Yaml, key: &str) -> Vec<String> {
    entry
        .get(key)
        .and_then(Yaml::as_sequence)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

mod pass;
