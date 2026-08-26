//! Everything that leaves this machine, why, and what stops if you refuse it.
//!
//! Two lists, and keeping them apart is most of the point. lemonfiber makes five
//! requests on its own account and they are enumerated here in full; the services
//! in the stack make a great many more, and those are **theirs** — an indexer
//! query is Prowlarr asking an indexer, a poster is Radarr asking a metadata
//! provider, and a peer connection is qBittorrent doing what a torrent client is.
//! Counting those as lemonfiber's would overstate what this product does; leaving
//! them out entirely would understate what running the stack does. So they are
//! listed, and listed as somebody else's.
//!
//! What makes this a surface rather than a comment is that an operator can read it.
//! A promise about network behaviour kept in a document is a promise; one an
//! operator can list, switch off one at a time, and be told the cost of switching
//! off is a property of the product.
//!
//! Where each entry goes is read from this machine as it is configured rather than
//! written down: the echo sources are the ones in force, the registries are the
//! ones the images in this stack name, and the indexer is wherever the operator
//! pointed it — with its query stripped, because an indexer authenticates by one.

mod ours;
mod theirs;

use serde::Serialize;

use crate::config::Settings;
use lemonfiber_manifest::Service;

pub use ours::{nothing_configured, EVERY, GUIDE_SOURCE};
pub use theirs::ELSEWHERE;

/// One of the requests lemonfiber makes on its own account.
///
/// Five, and the closed set is the claim. A sixth is a decision somebody makes by
/// adding a variant here and answering four questions about it, rather than one
/// that happens by somebody building a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Reach {
    /// Fetching the service images the stack runs.
    Registry,
    /// Probing the source the community quality guides are synced from.
    Guides,
    /// Asking what public address this machine's traffic comes out of.
    Echo,
    /// Proving an indexer key against the indexer.
    Indexer,
    /// Proving a Usenet login against the provider.
    Usenet,
}

impl Reach {
    /// The name this request is asked about and switched off by.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Registry => "registry",
            Self::Guides => "guides",
            Self::Echo => "echo",
            Self::Indexer => "indexer",
            Self::Usenet => "usenet",
        }
    }
}

/// One request lemonfiber makes, where it goes, and what refusing it costs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Outbound {
    /// Which request this is.
    pub reach: Reach,
    /// Where it goes as this machine is configured. Empty where nothing is
    /// configured to reach, which is not the same as switched off.
    pub destination: Vec<String>,
    /// Why lemonfiber asks.
    pub purpose: String,
    /// Exactly what travels in the request.
    pub sends: String,
    /// Whether this machine's settings allow it.
    pub allowed: bool,
    /// The setting that switches it off.
    pub switch: String,
    /// What stops working once it is off.
    pub cost: String,
}

/// A request one of the stack's services makes, which is not lemonfiber's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Elsewhere {
    /// The service, by the id the stack declares it under.
    pub service: String,
    /// Where its requests go, in the terms an operator would recognise.
    pub destination: String,
    /// What it asks for.
    pub purpose: String,
}

/// Everything that leaves this machine: lemonfiber's own requests, and the stack's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Leaving {
    /// Every request lemonfiber makes on its own account, in a fixed order.
    pub ours: Vec<Outbound>,
    /// The requests made by services this stack runs, attributed to them.
    pub theirs: Vec<Elsewhere>,
}

/// What leaves this machine, as it is configured and as the stack stands.
///
/// The services are taken from the manifest rather than from what is running,
/// because a service that is stopped still reaches the network the moment it is
/// started, and an operator deciding what they are comfortable with is deciding
/// about the stack rather than about this minute.
#[must_use]
pub fn leaving(settings: &Settings, services: &[Service]) -> Leaving {
    Leaving {
        ours: EVERY
            .iter()
            .map(|reach| ours::outbound(*reach, settings, services))
            .collect(),
        theirs: theirs::elsewhere(services),
    }
}

#[cfg(test)]
mod tests {
    use super::{leaving, Reach, EVERY};
    use crate::config::{Reaching, Settings};

    fn a_stack() -> Vec<lemonfiber_manifest::Service> {
        crate::test_support::stack()
            .manifest()
            .map(|manifest| manifest.services)
            .unwrap_or_default()
    }

    #[test]
    fn every_request_this_product_makes_is_listed_once() {
        let listed: Vec<Reach> = leaving(&Settings::default(), &a_stack())
            .ours
            .into_iter()
            .map(|entry| entry.reach)
            .collect();
        assert_eq!(listed, EVERY.to_vec());
        let mut sorted = listed.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), listed.len(), "a request is listed twice");
    }

    #[test]
    fn each_entry_says_where_it_goes_why_what_it_sends_and_what_refusing_costs() {
        for entry in leaving(&Settings::default(), &a_stack()).ours {
            let name = entry.reach.as_str();
            assert!(!name.is_empty());
            for (field, said) in [
                ("purpose", &entry.purpose),
                ("sends", &entry.sends),
                ("cost", &entry.cost),
            ] {
                assert!(
                    said.split_whitespace().count() >= 5,
                    "{name} says nothing useful about its {field}: {said}"
                );
            }
            // The switch is a setting an operator types, not a sentence: what it
            // owes is that it names one this product recognises, which is what a
            // list nobody can act on would be missing.
            assert!(
                crate::config::SETTINGS.contains(&entry.switch.as_str()),
                "{name} is switched off by {}, which is not a setting this product reads",
                entry.switch
            );
        }
    }

    #[test]
    fn a_machine_that_refuses_everything_says_so_of_every_entry() {
        let settings = Settings {
            ip_echo: Vec::new(),
            reaching: Reaching::none(),
            ..Settings::default()
        };
        let refused: Vec<Reach> = leaving(&settings, &a_stack())
            .ours
            .into_iter()
            .filter(|entry| entry.allowed)
            .map(|entry| entry.reach)
            .collect();
        assert!(refused.is_empty(), "these are still allowed: {refused:?}");
    }

    #[test]
    fn the_stacks_own_requests_are_listed_as_the_stacks() {
        let theirs = leaving(&Settings::default(), &a_stack()).theirs;
        assert!(!theirs.is_empty(), "the stack reaches the network");
        for entry in &theirs {
            assert!(!entry.service.is_empty());
            assert!(
                entry.purpose.split_whitespace().count() >= 5,
                "{} says nothing about what it asks for",
                entry.service
            );
        }
    }

    #[test]
    fn a_stack_with_no_services_leaves_the_stacks_own_list_empty() {
        let leaving = leaving(&Settings::default(), &[]);
        assert!(leaving.theirs.is_empty());
        assert_eq!(leaving.ours.len(), EVERY.len());
    }

    #[test]
    fn a_name_is_given_for_every_request() {
        let named: Vec<&str> = EVERY.iter().map(|reach| reach.as_str()).collect();
        assert_eq!(
            named,
            vec!["registry", "guides", "echo", "indexer", "usenet"]
        );
    }
}
