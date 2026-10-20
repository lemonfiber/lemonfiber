use lemonfiber_manifest::container::Network;

use super::super::Address;
use super::shipped;
use crate::test_support::{a_placed, an_installed};
use crate::wiring::Chosen;

const GUARD: &str = "network.egress-guard";

fn inside_the_guard(service: &str, listens: u16) -> crate::plugin::Placed {
    let mut placed = a_placed(service, &["download.torrent"], None, Some(listens));
    placed.fields.network = Some(Network {
        via: GUARD.to_owned(),
    });
    placed
}

#[test]
fn a_stack_service_taking_the_guard_names_it_and_one_taking_nothing_names_none() {
    let fillers = shipped(&[], &Chosen::default(), |_| ());
    assert_eq!(
        fillers
            .service("qbittorrent")
            .and_then(|one| one.via.clone()),
        Some(GUARD.to_owned())
    );
    assert_eq!(
        fillers.service("sabnzbd").map(|one| one.via.clone()),
        Some(None)
    );
}

#[test]
fn a_plugins_service_taking_the_guard_is_reached_at_the_guard() {
    let installed = [an_installed(
        "deluge",
        vec![inside_the_guard("deluge", 8112)],
    )];
    let fillers = shipped(&installed, &Chosen::default(), |_| ());
    let deluge = fillers.service("deluge");
    assert_eq!(
        deluge.and_then(|one| one.via.clone()),
        Some(GUARD.to_owned())
    );
    assert_eq!(
        deluge.and_then(|one| one.address.clone()),
        Some(Address {
            host: "gluetun".to_owned(),
            port: 8112
        })
    );
}

#[test]
fn a_plugins_service_taking_a_guard_nothing_fills_stays_at_its_own_id() {
    let installed = [an_installed(
        "deluge",
        vec![inside_the_guard("deluge", 8112)],
    )];
    let fillers = shipped(&installed, &Chosen::default(), |manifest| {
        manifest
            .services
            .retain(|service| !service.provides.iter().any(|named| named == GUARD));
    });
    assert_eq!(
        fillers
            .service("deluge")
            .and_then(|one| one.address.as_ref())
            .map(|at| at.host.clone()),
        Some("deluge".to_owned())
    );
}
