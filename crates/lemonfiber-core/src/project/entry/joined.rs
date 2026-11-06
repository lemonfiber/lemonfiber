//! The networks a service joins, as its entry writes them.

use std::collections::BTreeMap;
use std::net::Ipv4Addr;

use serde::ser::SerializeMap;
use serde::Serialize;

use super::literal;

/// The networks a service joins: by name alone where none fixes its address and nothing
/// else answers through it, and otherwise each with the address it holds where one does
/// and the names it answers to there.
pub(super) struct Joined {
    /// Each network, with the address it holds there where fixed.
    at: BTreeMap<String, Option<Ipv4Addr>>,
    /// The ids of the services running inside its network, which answer through it.
    aliases: Vec<String>,
    /// The further names it answers to on one network alone, by that network.
    answers: BTreeMap<String, Vec<String>>,
}

impl Joined {
    /// Each of `networks`, with the address it holds there where fixed, answering to
    /// nothing beyond its own name.
    pub(super) fn on(networks: impl IntoIterator<Item = (String, Option<Ipv4Addr>)>) -> Self {
        Self {
            at: networks
                .into_iter()
                .map(|(name, at)| (literal(&name), at))
                .collect(),
            aliases: Vec::new(),
            answers: BTreeMap::new(),
        }
    }

    /// Whether it joins none.
    pub(super) fn is_empty(&self) -> bool {
        self.at.is_empty()
    }

    /// Answer on every network it joins to the ids of the services `inside` its network.
    pub(super) fn answer_for(&mut self, inside: &[String]) {
        self.aliases = inside.iter().map(|id| literal(id)).collect();
    }

    /// Answer on the network `network` to each of `hosts` as well: written where it joins
    /// that network.
    pub(super) fn answer_as(&mut self, network: &str, hosts: &[&str]) {
        self.answers
            .entry(literal(network))
            .or_default()
            .extend(hosts.iter().map(|host| literal(host)));
    }
}

impl Serialize for Joined {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.aliases.is_empty()
            && self.answers.is_empty()
            && self.at.values().all(Option::is_none)
        {
            return serializer.collect_seq(self.at.keys());
        }
        let mut map = serializer.serialize_map(Some(self.at.len()))?;
        for (name, address) in &self.at {
            let aliases: Vec<String> = self
                .aliases
                .iter()
                .chain(self.answers.get(name).into_iter().flatten())
                .cloned()
                .collect();
            let holds = Holds {
                ipv4_address: *address,
                aliases: &aliases,
            };
            map.serialize_entry(name, &holds)?;
        }
        map.end()
    }
}

/// One network's settings for a service: the address it holds there, where fixed, and the
/// names it answers to there.
#[derive(Serialize)]
struct Holds<'a> {
    /// Its fixed address.
    #[serde(skip_serializing_if = "Option::is_none")]
    ipv4_address: Option<Ipv4Addr>,
    /// The names it answers to there beyond its own.
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    aliases: &'a [String],
}
