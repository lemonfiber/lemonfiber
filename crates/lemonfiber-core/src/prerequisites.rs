//! What the operator has to bring, worked out from what they asked for.
//!
//! The accounts a media stack needs are the wall a non-technical operator hits
//! last and hardest: everything installs, nothing downloads, and no error says
//! why. So the list is derived up front from the download protocols they chose,
//! costed, and explained — before a single credential is requested.
//!
//! This is a pure function of that choice. It names no vendors, because a
//! recommendation ages badly and varies by region; it describes the criteria
//! that actually decide the choice instead. The one criterion with a lasting
//! consequence for torrents — whether the VPN forwards a port — is stated here,
//! while the operator can still act on it, rather than after they have paid for
//! a year of something that cannot do it.
//!
//! The wizard renders this and walks it; nothing here prompts or persists.

use serde::Serialize;

use crate::config::Protocols;

/// Roughly what a prerequisite costs, in bands rather than prices.
///
/// Bands rather than figures, because a price printed in the binary is wrong
/// within a year and varies by region — the same reasons vendors are not named.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Cost {
    /// No third-party spend at all.
    Free,
    /// Free to start, with paid tiers to lift its limits.
    Freemium,
    /// A recurring subscription.
    Subscription,
}

impl Cost {
    /// The band as a short phrase an operator reads.
    #[must_use]
    pub const fn phrase(self) -> &'static str {
        match self {
            Self::Free => "usually free to join, though some are invite-only",
            Self::Freemium => "free with limits, or a small recurring fee to lift them",
            Self::Subscription => "a recurring subscription, sometimes sold as a one-off allowance",
        }
    }
}

/// One thing the operator must obtain before a protocol will work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Prerequisite {
    /// A stable identifier for the thing, such as `usenet.provider`.
    pub id: &'static str,
    /// What it is called, in the operator's terms.
    pub label: &'static str,
    /// What it is, in plain language.
    pub what: &'static str,
    /// Why the stack needs it.
    pub why: &'static str,
    /// Roughly what it costs.
    pub cost: Cost,
    /// What actually decides the choice between one and another — criteria, not
    /// vendors.
    pub criteria: Vec<&'static str>,
    /// What is lost if this one is skipped, so the consequence is stated rather
    /// than discovered.
    pub without: &'static str,
}

/// Everything the operator's protocol choices require of them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PrerequisiteMap {
    /// The choices this was derived from.
    pub protocols: Protocols,
    /// The prerequisites, in the order they should be obtained.
    pub items: Vec<Prerequisite>,
    /// The plain statement that a library-only setup needs nothing, present only
    /// when nothing is required — a legitimate, valuable, zero-cost end state
    /// rather than a degraded one.
    pub library_only: Option<&'static str>,
}

/// The prerequisites the given protocol choices imply.
///
/// Nothing chosen needs nothing bought: a folder of existing media reaches a
/// working Jellyfin with no accounts, and that path is stated rather than left
/// for the operator to infer. Each chosen protocol adds exactly its own
/// prerequisites and no others, in the order they have to be obtained — a
/// provider before the indexer that searches it produces a confusing partial
/// state otherwise.
#[must_use]
pub fn prerequisites(protocols: Protocols) -> PrerequisiteMap {
    let mut items = Vec::new();
    if protocols.usenet {
        items.push(usenet_provider());
        items.push(usenet_indexer());
    }
    if protocols.torrent {
        items.push(torrent_indexer());
        items.push(vpn());
    }

    PrerequisiteMap {
        protocols,
        library_only: items.is_empty().then_some(LIBRARY_ONLY),
        items,
    }
}

/// The zero-cost path, stated whenever nothing else is required.
const LIBRARY_ONLY: &str = "A library-only setup needs no third-party accounts. \
     An existing folder of media reaches a working Jellyfin with nothing bought — \
     a supported end state, not a lesser one.";

/// The Usenet provider: where the files actually are.
fn usenet_provider() -> Prerequisite {
    Prerequisite {
        id: "usenet.provider",
        label: "Usenet provider",
        what: "A subscription that gives you access to the Usenet servers where files are stored.",
        why: "It holds the data your downloaders fetch. Without one, Usenet has nothing to download from.",
        cost: Cost::Subscription,
        criteria: vec![
            "retention — how far back its history reaches",
            "how many simultaneous connections it allows",
            "which backbone it runs on, which affects how often downloads complete",
            "whether it is an unlimited account or a one-off block of data",
        ],
        without: "Usenet downloads cannot run at all.",
    }
}

/// The Usenet indexer: how anything on Usenet is found. The confusing half of
/// the most-confused pair in the whole setup.
fn usenet_indexer() -> Prerequisite {
    Prerequisite {
        id: "usenet.indexer",
        label: "Usenet indexer",
        what: "A search service that tells your downloaders where on Usenet a given file is.",
        why: "The provider stores the files; the indexer is how anything finds them. They are two different services, and confusing the two is the single most common mistake in this domain — both are required.",
        cost: Cost::Freemium,
        criteria: vec![
            "the range of content it covers",
            "how quickly newly posted content appears",
            "its daily search and download limits",
        ],
        without: "Usenet searches return nothing, even with a provider configured.",
    }
}

/// The torrent indexer or tracker: how torrents and their peers are found.
fn torrent_indexer() -> Prerequisite {
    Prerequisite {
        id: "torrent.indexer",
        label: "Torrent indexer",
        what: "A tracker or indexer that lists torrents and coordinates the peers sharing them.",
        why: "It is how your downloaders find torrents, and the peers to fetch each one from.",
        cost: Cost::Free,
        criteria: vec![
            "the range of content it covers",
            "whether it is open to join or invite-only",
            "on private trackers, the sharing ratio it expects you to keep",
        ],
        without: "Torrent searches return nothing.",
    }
}

/// The VPN: what keeps the operator's home connection out of every peer's sight.
/// Its port-forwarding criterion is the one with a lasting consequence.
fn vpn() -> Prerequisite {
    Prerequisite {
        id: "vpn",
        label: "VPN",
        what: "A virtual private network that carries your torrent traffic through an encrypted tunnel.",
        why: "With torrents, your address is visible to every peer you connect to. The VPN is what keeps your home connection from being one of them.",
        cost: Cost::Subscription,
        criteria: vec![
            "whether it forwards a port — without it, peers cannot connect back to you, so throughput drops and seeding suffers, which matters where a ratio affects your standing",
            "whether it permits peer-to-peer traffic at all",
            "how many devices one subscription covers",
        ],
        without: "Torrents can still run, but your home address is visible to peers; lemonfiber warns and asks you to confirm before allowing it.",
    }
}

#[cfg(test)]
mod tests {
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
            for item in prerequisites(protocols).items {
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
        for item in prerequisites(Protocols::both()).items {
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
}
