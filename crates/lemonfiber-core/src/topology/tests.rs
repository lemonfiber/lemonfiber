use std::collections::{BTreeMap, BTreeSet};
use std::net::Ipv4Addr;

use lemonfiber_contract::capabilities::media::serve::CAPABILITY as MEDIA_SERVE;

use super::{
    derived, Block, Member, Named, Placed, Settled, Topology, DOOR_ON_MEDIA, DOOR_ON_PROXY,
    MEDIA_SUBNET, PROXY_FRONT, PROXY_ON_PROXY, PROXY_SUBNET,
};

const fn member(id: &str) -> Member<'_> {
    Member {
        id,
        confined: false,
        publishes: false,
        inside: None,
        fronts: None,
    }
}

const fn ask<'a>(asker: &'a str, capability: &'a str, reached: &'a str) -> Settled<'a> {
    Settled {
        asker,
        capability,
        reached,
    }
}

fn on(topology: &Topology, id: &str) -> BTreeMap<String, Option<Ipv4Addr>> {
    match topology.placed.get(id) {
        Some(Placed::On(on)) => on.clone(),
        _ => BTreeMap::new(),
    }
}

fn names(topology: &Topology, id: &str) -> Vec<String> {
    on(topology, id).into_keys().collect()
}

fn joined(topology: &Topology) -> BTreeMap<String, BTreeSet<String>> {
    let mut joined: BTreeMap<String, BTreeSet<String>> = topology
        .networks
        .keys()
        .map(|name| (name.clone(), BTreeSet::new()))
        .collect();
    for (id, placed) in &topology.placed {
        if let Placed::On(on) = placed {
            for name in on.keys() {
                joined.entry(name.clone()).or_default().insert(id.clone());
            }
        }
    }
    joined
}

fn spelled(networks: &[(&str, &[&str])]) -> BTreeMap<String, BTreeSet<String>> {
    networks
        .iter()
        .map(|(name, ids)| {
            (
                (*name).to_owned(),
                ids.iter().map(|id| (*id).to_owned()).collect(),
            )
        })
        .collect()
}

fn holds(block: Block, address: Ipv4Addr) -> bool {
    let mask = u32::MAX
        .checked_shl(u32::BITS - u32::from(block.1))
        .unwrap_or(0);
    u32::from(address) & mask == u32::from(block.0) & mask
}

#[test]
fn every_service_that_is_not_confined_has_a_network_of_its_own_with_a_route_off_the_host() {
    let topology = derived(&[member("sonarr")], &[], &[], None);
    assert_eq!(names(&topology, "sonarr"), ["own__sonarr"]);
    let own = topology.networks.get("own__sonarr");
    assert!(own.is_some_and(|network| !network.internal && network.masquerade));
}

#[test]
fn a_confined_service_is_on_its_own_network_only_where_it_publishes_and_then_translates_nothing() {
    let quiet = Member {
        confined: true,
        ..member("gate")
    };
    let answering = Member {
        confined: true,
        publishes: true,
        ..member("decline")
    };
    let topology = derived(&[quiet, answering], &[], &[], None);
    assert_eq!(topology.placed.get("gate"), Some(&Placed::Nowhere));
    let own = topology.networks.get("own__decline");
    assert!(own.is_some_and(|network| !network.internal && !network.masquerade));
}

#[test]
fn each_settled_ask_is_a_network_of_its_own_joining_the_asker_and_the_reached_alone() {
    let members = [member("seerr"), member("sonarr"), member("radarr")];
    let asks = [
        ask("seerr", "library.curate", "sonarr"),
        ask("seerr", "library.curate", "radarr"),
    ];
    let topology = derived(&members, &asks, &[], None);
    assert_eq!(
        names(&topology, "seerr"),
        [
            "ask__seerr__library.curate__radarr",
            "ask__seerr__library.curate__sonarr",
            "own__seerr"
        ]
    );
    assert_eq!(
        names(&topology, "sonarr"),
        ["ask__seerr__library.curate__sonarr", "own__sonarr"]
    );
    assert!(!names(&topology, "radarr").contains(&"ask__seerr__library.curate__sonarr".to_owned()));
    assert!(topology
        .networks
        .get("ask__seerr__library.curate__sonarr")
        .is_some_and(|network| network.internal));
}

#[test]
fn an_adapter_and_its_upstream_share_a_network_nothing_else_joins() {
    let adapter = Member {
        confined: true,
        fronts: Some("jellyfin-server"),
        ..member("jellyfin")
    };
    let topology = derived(
        &[adapter, member("jellyfin-server"), member("seerr")],
        &[],
        &[],
        None,
    );
    assert_eq!(names(&topology, "jellyfin"), ["front__jellyfin"]);
    assert!(names(&topology, "jellyfin-server").contains(&"front__jellyfin".to_owned()));
    assert!(!names(&topology, "seerr").contains(&"front__jellyfin".to_owned()));
}

#[test]
fn a_service_inside_another_has_no_network_and_an_ask_of_it_reaches_its_host() {
    let client = Member {
        inside: Some("gluetun"),
        ..member("qbittorrent")
    };
    let members = [member("gluetun"), client, member("sonarr")];
    let asks = [
        ask("qbittorrent", "network.egress-guard", "gluetun"),
        ask("sonarr", "download.torrent", "qbittorrent"),
    ];
    let topology = derived(&members, &asks, &[], None);
    assert_eq!(
        topology.placed.get("qbittorrent"),
        Some(&Placed::Inside("gluetun".to_owned()))
    );
    assert!(
        names(&topology, "gluetun").contains(&"ask__sonarr__download.torrent__gluetun".to_owned())
    );
    assert!(!topology
        .networks
        .keys()
        .any(|name| name.contains("egress-guard")));
}

#[test]
fn a_link_by_name_joins_the_service_it_names() {
    let topology = derived(
        &[member("homepage"), member("sonarr")],
        &[],
        &[Named {
            by: "homepage",
            to: "sonarr",
        }],
        None,
    );
    assert!(names(&topology, "homepage").contains(&"link__homepage__sonarr".to_owned()));
    assert!(names(&topology, "sonarr").contains(&"link__homepage__sonarr".to_owned()));
}

#[test]
fn the_door_holds_fixed_addresses_on_its_asks_of_the_proxy_and_the_media_server() {
    let door = Member {
        confined: true,
        publishes: true,
        ..member("door")
    };
    let members = [door, member("caddy"), member("jellyfin")];
    let asks = [
        ask("door", PROXY_FRONT, "caddy"),
        ask("door", MEDIA_SERVE, "jellyfin"),
    ];
    let topology = derived(&members, &asks, &[], Some("door"));
    let door_on = on(&topology, "door");
    assert_eq!(
        door_on.get("ask__door__proxy.front__caddy"),
        Some(&Some(DOOR_ON_PROXY))
    );
    assert_eq!(
        door_on.get("ask__door__media.serve__jellyfin"),
        Some(&Some(DOOR_ON_MEDIA))
    );
    assert_eq!(
        on(&topology, "caddy").get("ask__door__proxy.front__caddy"),
        Some(&Some(PROXY_ON_PROXY))
    );
    assert_eq!(
        on(&topology, "jellyfin").get("ask__door__media.serve__jellyfin"),
        Some(&None)
    );
    assert!(topology
        .networks
        .get("ask__door__media.serve__jellyfin")
        .and_then(|network| network.fixed)
        .is_some_and(|subnet| subnet == MEDIA_SUBNET));
    assert!(topology
        .networks
        .get("ask__door__proxy.front__caddy")
        .and_then(|network| network.fixed)
        .is_some_and(|subnet| subnet == PROXY_SUBNET));
    let unguarded = derived(&members, &asks, &[], None);
    assert!(unguarded
        .networks
        .values()
        .all(|network| network.fixed.is_none()));
}

#[test]
fn no_network_joins_services_that_no_rule_joins() {
    let adapter = Member {
        confined: true,
        fronts: Some("jellyfin-server"),
        ..member("jellyfin")
    };
    let client = Member {
        inside: Some("gluetun"),
        ..member("qbittorrent")
    };
    let quiet = Member {
        confined: true,
        ..member("gate")
    };
    let members = [
        member("seerr"),
        member("sonarr"),
        member("jellyfin-server"),
        adapter,
        member("gluetun"),
        client,
        quiet,
    ];
    let asks = [
        ask("seerr", "library.curate", "sonarr"),
        ask("seerr", MEDIA_SERVE, "jellyfin"),
        ask("sonarr", "download.torrent", "qbittorrent"),
        ask("qbittorrent", "network.egress-guard", "gluetun"),
    ];
    let links = [Named {
        by: "sonarr",
        to: "qbittorrent",
    }];
    let topology = derived(&members, &asks, &links, None);
    assert_eq!(
        joined(&topology),
        spelled(&[
            ("ask__seerr__library.curate__sonarr", &["seerr", "sonarr"]),
            ("ask__seerr__media.serve__jellyfin", &["jellyfin", "seerr"]),
            (
                "ask__sonarr__download.torrent__gluetun",
                &["gluetun", "sonarr"]
            ),
            ("front__jellyfin", &["jellyfin", "jellyfin-server"]),
            ("link__sonarr__gluetun", &["gluetun", "sonarr"]),
            ("own__gluetun", &["gluetun"]),
            ("own__jellyfin-server", &["jellyfin-server"]),
            ("own__seerr", &["seerr"]),
            ("own__sonarr", &["sonarr"]),
        ])
    );
    assert_eq!(topology.placed.get("gate"), Some(&Placed::Nowhere));
}

#[test]
fn a_link_by_name_within_one_network_namespace_is_no_network() {
    let client = Member {
        inside: Some("gluetun"),
        ..member("qbittorrent")
    };
    let topology = derived(
        &[member("gluetun"), client],
        &[],
        &[Named {
            by: "qbittorrent",
            to: "gluetun",
        }],
        None,
    );
    assert_eq!(names(&topology, "gluetun"), ["own__gluetun"]);
    assert_eq!(topology.networks.len(), 1);
}

#[test]
fn the_door_holds_no_fixed_address_on_an_ask_of_anything_else() {
    let members = [member("door"), member("tinyauth")];
    let asks = [ask("door", "identity.source", "tinyauth")];
    let topology = derived(&members, &asks, &[], Some("door"));
    assert_eq!(
        on(&topology, "door").get("ask__door__identity.source__tinyauth"),
        Some(&None)
    );
    assert!(topology
        .networks
        .values()
        .all(|network| network.fixed.is_none()));
}

#[test]
fn each_fixed_address_lies_in_its_subnet_where_the_engine_hands_out_no_other() {
    for (subnet, addresses) in [
        (PROXY_SUBNET, &[DOOR_ON_PROXY, PROXY_ON_PROXY][..]),
        (MEDIA_SUBNET, &[DOOR_ON_MEDIA][..]),
    ] {
        assert!(holds(subnet.whole, subnet.handed_out.0));
        assert!(subnet.handed_out.1 > subnet.whole.1);
        for address in addresses {
            assert!(holds(subnet.whole, *address), "{address}");
            assert!(!holds(subnet.handed_out, *address), "{address}");
        }
    }
    assert!(!holds(PROXY_SUBNET.whole, MEDIA_SUBNET.whole.0));
    assert!(!holds(MEDIA_SUBNET.whole, PROXY_SUBNET.whole.0));
}

#[test]
fn two_different_asks_never_spell_one_network() {
    let members = [member("komga-server"), member("komga"), member("z")];
    let asks = [
        ask("komga-server", "x.y", "z"),
        ask("komga", "server-x.y", "z"),
    ];
    let topology = derived(&members, &asks, &[], None);
    let asked: Vec<&String> = topology
        .networks
        .keys()
        .filter(|name| name.starts_with("ask"))
        .collect();
    assert_eq!(asked.len(), 2, "{asked:?}");
    assert_eq!(super::named(&["a_b", "c"]), "a_5f_b__c");
    assert_ne!(super::named(&["a", "b__c"]), super::named(&["a__b", "c"]));
}
