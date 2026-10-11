use std::collections::BTreeMap;
use std::net::Ipv4Addr;

use super::{derived, Member, Named, Placed, Settled, Topology, MEDIA_SERVE, PROXY_FRONT};

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
        Some(&Some(Ipv4Addr::new(10, 80, 96, 2)))
    );
    assert_eq!(
        door_on.get("ask__door__media.serve__jellyfin"),
        Some(&Some(super::trusted_by_media()))
    );
    assert_eq!(
        on(&topology, "caddy").get("ask__door__proxy.front__caddy"),
        Some(&Some(super::trusted_by_door()))
    );
    assert_eq!(
        on(&topology, "jellyfin").get("ask__door__media.serve__jellyfin"),
        Some(&None)
    );
    assert!(topology
        .networks
        .get("ask__door__media.serve__jellyfin")
        .and_then(|network| network.fixed)
        .is_some_and(|subnet| subnet.whole == (Ipv4Addr::new(10, 80, 96, 16), 28)));
    let unguarded = derived(&members, &asks, &[], None);
    assert!(unguarded
        .networks
        .values()
        .all(|network| network.fixed.is_none()));
}

#[test]
fn no_network_joins_services_that_no_rule_joins() {
    let members = [member("a"), member("b"), member("c")];
    let topology = derived(&members, &[ask("a", "x.y", "b")], &[], None);
    for name in topology.networks.keys() {
        let joined: Vec<&String> = topology
            .placed
            .iter()
            .filter(|(_, placed)| matches!(placed, Placed::On(on) if on.contains_key(name)))
            .map(|(id, _)| id)
            .collect();
        assert!(
            !joined.contains(&&"c".to_owned()) || name == "own__c",
            "{name}: {joined:?}"
        );
    }
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
