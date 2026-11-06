use std::collections::BTreeMap;

use lemonfiber_manifest::Manifest;

use super::Fronting;
use crate::topology::{Placed, Topology};
use crate::wiring::Chosen;

/// The stack this build carries beside Plex moved into its plugin, the request gate asking
/// for it in Plex's API and the request service moved into its plugin.
fn beside_plex() -> (Manifest, Vec<crate::plugin::Installed>) {
    let Ok(manifest) = Manifest::from_toml(lemonfiber_bundled::STACK) else {
        unreachable!("the embedded stack parses");
    };
    let installed = crate::test_support::plexing::plexed()
        .into_iter()
        .chain(
            crate::test_support::guards::GATE
                .installed()
                .map(crate::test_support::plexing::asking_in_plex_too),
        )
        .chain(crate::test_support::requests::REQUESTS.installed())
        .collect();
    (manifest, installed)
}

/// A service on each of `networks`, with no fixed address.
fn on(networks: &[&str]) -> Placed {
    Placed::On(
        networks
            .iter()
            .map(|network| ((*network).to_owned(), None))
            .collect(),
    )
}

#[test]
fn a_gate_running_inside_another_answers_on_that_ones_networks_shared_with_the_request_service() {
    let (manifest, installed) = beside_plex();
    let topology = Topology {
        networks: BTreeMap::new(),
        placed: [
            ("fence".to_owned(), on(&["asking", "plex"])),
            ("seerr".to_owned(), on(&["asking"])),
        ]
        .into(),
    };
    let placed = |id: &str| match id {
        "request-gate" => Placed::Inside("fence".to_owned()),
        _ => topology.placed.get(id).cloned().unwrap_or(Placed::Nowhere),
    };
    let fronted = |name: &str| (name == crate::plex_link::SETTING).then(|| "fronted".to_owned());

    let fronting = Fronting::of(
        (&manifest, &installed, &Chosen::read(None)),
        &fronted,
        &topology,
        &placed,
    );

    assert_eq!(
        fronting,
        Some(Fronting {
            answering: "fence".to_owned(),
            networks: vec!["asking".to_owned()],
            requests: "seerr".to_owned(),
        })
    );
}

#[test]
fn a_request_service_on_no_network_shares_none_with_the_gate() {
    let (manifest, installed) = beside_plex();
    let topology = Topology {
        networks: BTreeMap::new(),
        placed: [("request-gate".to_owned(), on(&["asking"]))].into(),
    };
    let placed = |id: &str| topology.placed.get(id).cloned().unwrap_or(Placed::Nowhere);
    let fronted = |name: &str| (name == crate::plex_link::SETTING).then(|| "fronted".to_owned());

    let fronting = Fronting::of(
        (&manifest, &installed, &Chosen::read(None)),
        &fronted,
        &topology,
        &placed,
    );

    assert_eq!(fronting.map(|one| one.networks), Some(Vec::new()));
}
