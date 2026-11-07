use super::joined;
use crate::wiring::{Reaches, Settled, Wired};

/// `by`'s ask for `capability`, reaching `services`.
fn asking(by: &str, capability: &str, services: &[&str]) -> Wired {
    Wired {
        by: by.to_owned(),
        origin: crate::origin::Origin::Bundled,
        reaches: Reaches::Asked {
            capability: capability.to_owned(),
            services: services.iter().map(|one| (*one).to_owned()).collect(),
            settled: Settled::Outright,
            origins: std::collections::BTreeMap::new(),
            native: None,
            files: None,
        },
    }
}

/// What `settled` joins, each as who asked what and what it reaches.
fn networks(settled: Vec<Wired>) -> Vec<(String, String, String)> {
    joined(settled, &[])
        .0
        .into_iter()
        .map(|ask| (ask.asker, ask.capability, ask.reached))
        .collect()
}

/// One network, as [`networks`] says it.
fn network(asker: &str, capability: &str, reached: &str) -> (String, String, String) {
    (asker.to_owned(), capability.to_owned(), reached.to_owned())
}

#[test]
fn a_service_the_request_gate_stands_in_front_of_joins_no_network_with_a_curator() {
    let joined = networks(vec![
        asking("requests", "library.curate", &["curator"]),
        asking("requests", "request.gate", &["gate"]),
        asking("gate", "library.curate", &["curator"]),
    ]);

    assert_eq!(
        joined,
        vec![
            network("requests", "request.gate", "gate"),
            network("gate", "library.curate", "curator"),
        ]
    );
}

#[test]
fn a_service_asking_a_gate_nothing_fills_keeps_its_ask_of_a_curator() {
    let joined = networks(vec![
        asking("requests", "library.curate", &["curator"]),
        asking("requests", "request.gate", &[]),
    ]);

    assert_eq!(
        joined,
        vec![network("requests", "library.curate", "curator")]
    );
}
