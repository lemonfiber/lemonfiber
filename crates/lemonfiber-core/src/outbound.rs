//! Everything that leaves this machine, why, and what stops if you refuse it.
//!
//! Two lists, and keeping them apart is most of the point. lemonfiber makes seven
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

pub use ours::{nothing_configured, EVERY, GUIDE_SOURCE, PUSHBULLET, PUSHOVER, RELEASE_LIST};

/// One of the requests lemonfiber makes on its own account.
///
/// Seven, and the closed set is the claim. An eighth is a decision somebody makes by
/// adding a variant here and answering four questions about it, rather than one
/// that happens by somebody building a request.
///
/// The sixth is the one that carries somebody's words rather than a credential or a
/// name, and it was added deliberately and late: five of these prove or fetch
/// something and could say what travels in a phrase, and this one travels to a
/// person. What that costs is the same four answers as the rest, and one more thing
/// the others do not owe — it is the only entry whose destination is somebody else's
/// choice, so the list names the two services it can reach and the sender is handed
/// them rather than holding addresses of its own.
///
/// The seventh is the only one this program makes about *itself*, and it is the one
/// whose absence used to be the claim. What it costs to allow is the shortest answer
/// on the list: it carries nothing at all, so the only thing switching it off keeps
/// from anybody is the knowledge that a version came out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "OutboundReach")]
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
    /// Telling a household member the one thing the request service cannot carry.
    Household,
    /// Asking which version of lemonfiber itself has been released.
    Updates,
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
            Self::Household => "household",
            Self::Updates => "updates",
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
    ///
    /// Empty means it reaches nothing, which is an answer. It never means *and we
    /// do not know*: that is [`Self::recorded`], and the two must not be read as one
    /// — an unknown service rendered as an empty destination would be this product
    /// claiming nothing leaves the machine on the strength of having no idea.
    pub destination: String,
    /// What it asks for.
    pub purpose: String,
    /// Whether lemonfiber ships a record of what this service reaches.
    ///
    /// False for a service that arrived in the stack after this build was made, or
    /// from an operator's own fork. It is listed anyway, because the alternative —
    /// leaving it out — is a privacy inventory that is complete-looking and short,
    /// and a reader counting the services on their machine against the ones on this
    /// list is the reader this surface exists for.
    pub recorded: bool,
    /// Whose request it is: the stack's own, or an installed plugin's, named.
    ///
    /// A column in this account rather than an account of its own, because what leaves
    /// this machine is one question however many parties are asking it.
    pub origin: crate::origin::Origin,
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
///
/// Every installed plugin is part of the stack for this, read from its record: each of
/// its services, and every destination its recipes declare outside it.
#[must_use]
pub fn leaving(
    settings: &Settings,
    services: &[Service],
    installed: &[crate::plugin::Installed],
) -> Leaving {
    let mut theirs = theirs::elsewhere(services);
    theirs.extend(theirs::brought(services, installed));
    Leaving {
        ours: EVERY
            .iter()
            .map(|reach| ours::outbound(*reach, settings, services))
            .collect(),
        theirs,
    }
}

#[cfg(test)]
mod tests;
