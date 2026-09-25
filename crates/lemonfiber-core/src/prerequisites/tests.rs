use super::{prerequisites, Cost};
use crate::config::Protocols;

/// The identifiers a map lists, in order.
fn ids(protocols: Protocols) -> Vec<&'static str> {
    prerequisites(protocols)
        .items
        .into_iter()
        .map(|item| item.id)
        .collect()
}

#[test]
fn nothing_chosen_needs_nothing_bought() {
    let map = prerequisites(Protocols::none());
    assert!(map.items.is_empty());
    let note = map.library_only.unwrap_or_default();
    assert!(
        note.contains("no third-party accounts"),
        "the zero-cost path is stated plainly: {note:?}"
    );
}

#[test]
fn a_chosen_protocol_adds_only_its_own_prerequisites() {
    assert_eq!(
        ids(Protocols {
            usenet: true,
            torrent: false,
        }),
        vec!["usenet.provider", "usenet.indexer"],
        "a usenet-only operator is never shown the VPN they declined",
    );
    assert_eq!(
        ids(Protocols {
            usenet: false,
            torrent: true,
        }),
        vec!["torrent.indexer", "vpn"],
    );
}

#[test]
fn a_required_map_does_not_repeat_the_zero_cost_statement() {
    let map = prerequisites(Protocols {
        usenet: true,
        torrent: false,
    });
    assert_eq!(map.library_only, None);
}

#[test]
fn both_protocols_list_the_provider_before_the_indexer_that_searches_it() {
    let ids = ids(Protocols::both());
    assert_eq!(
        ids,
        vec![
            "usenet.provider",
            "usenet.indexer",
            "torrent.indexer",
            "vpn"
        ],
    );
    let provider = ids.iter().position(|id| *id == "usenet.provider");
    let indexer = ids.iter().position(|id| *id == "usenet.indexer");
    assert!(provider < indexer, "order is enforced because it matters");
}

#[test]
fn the_indexer_is_told_apart_from_the_provider_it_is_confused_with() {
    let map = prerequisites(Protocols {
        usenet: true,
        torrent: false,
    });
    let indexer = map
        .items
        .iter()
        .find(|item| item.id == "usenet.indexer")
        .map(|item| item.why)
        .unwrap_or_default();
    assert!(
        indexer.contains("provider") && indexer.contains("indexer"),
        "the distinction is spelled out where the two meet: {indexer:?}",
    );
}

#[test]
fn the_vpn_states_port_forwarding_and_its_consequence() {
    let map = prerequisites(Protocols {
        usenet: false,
        torrent: true,
    });
    let vpn = map.items.iter().find(|item| item.id == "vpn");
    let forwarding = vpn
        .and_then(|item| {
            item.criteria
                .iter()
                .find(|criterion| criterion.contains("port"))
        })
        .copied()
        .unwrap_or_default();
    assert!(
        forwarding.contains("seeding") || forwarding.contains("throughput"),
        "the consequence is stated in plain terms, not just the property: {forwarding:?}",
    );
    let without = vpn.map(|item| item.without).unwrap_or_default();
    assert!(
        without.contains("visible to peers"),
        "skipping it names what is lost: {without:?}",
    );
}

#[test]
fn every_prerequisite_explains_itself_and_costs_itself() {
    for protocols in [
        Protocols::both(),
        Protocols {
            usenet: true,
            torrent: false,
        },
    ] {
        let items = prerequisites(protocols).items;
        // A protocol that needs nothing is an answer this file has a shape for —
        // `library_only` — so a set of items that came back empty here means the
        // walk below asked nothing of anything.
        assert!(!items.is_empty(), "{protocols:?} listed no prerequisite");
        for item in items {
            assert!(
                !item.what.is_empty(),
                "{} has no plain description",
                item.id
            );
            assert!(!item.why.is_empty(), "{} says no why", item.id);
            assert!(!item.without.is_empty(), "{} names no consequence", item.id);
            assert!(!item.criteria.is_empty(), "{} lists no criteria", item.id);
            assert!(
                !item.cost.phrase().is_empty(),
                "{} has no cost band",
                item.id
            );
        }
    }
}

#[test]
fn guidance_names_no_vendors_and_links_nowhere() {
    // Recommending or linking a provider is exactly what this must not do;
    // criteria describe the choice instead. A URL is the easiest way that
    // rule breaks, so it is the one asserted against.
    let items = prerequisites(Protocols::both()).items;
    assert!(!items.is_empty(), "nothing was checked for a vendor's name");
    for item in items {
        let corpus = format!("{} {} {}", item.what, item.why, item.criteria.join(" "));
        assert!(
            !corpus.contains("http") && !corpus.contains("www."),
            "{} points somewhere it should only describe: {corpus:?}",
            item.id,
        );
    }
}

#[test]
fn each_cost_band_reads_as_something() {
    for band in [Cost::Free, Cost::Freemium, Cost::Subscription] {
        assert!(!band.phrase().is_empty());
    }
}

#[test]
fn a_map_serialises_with_its_choices_and_its_items() {
    let map = prerequisites(Protocols {
        usenet: false,
        torrent: true,
    });
    let json = serde_json::to_string(&map).unwrap_or_default();
    assert!(json.contains(r#""cost":"subscription""#), "{json}");
    assert!(json.contains(r#""id":"vpn""#), "{json}");
    assert!(json.contains(r#""torrent":true"#), "{json}");
}
