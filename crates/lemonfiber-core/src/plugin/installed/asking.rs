//! What a placed service asks for.

use std::collections::BTreeMap;

use lemonfiber_manifest::container::{Network, Value};
use lemonfiber_plugin::{Manifest, Service};
use serde::{Deserialize, Serialize};

/// Every capability a service asks for: each `[[ask]]` it makes, then each it asks of its
/// container fields that none of those already asks.
pub(super) fn asks(manifest: &Manifest, service: &Service) -> Vec<Asking> {
    let declared: Vec<Asking> = manifest
        .asking
        .iter()
        .filter(|ask| {
            manifest
                .asks(ask.service.as_deref())
                .is_some_and(|asker| asker.id == service.id)
        })
        .map(|ask| Asking {
            capability: ask.capability.clone(),
            each: ask.each,
            native: ask.native.clone(),
            files: None,
        })
        .collect();
    merged(declared, &service.env, service.network.as_ref())
}

/// Each ask `env` and `network` make, as [`merged`] finds them.
pub(crate) fn implied(env: &BTreeMap<String, Value>, network: Option<&Network>) -> Vec<Asking> {
    merged(Vec::new(), env, network)
}

/// `asks`, with each ask `env` and `network` make that asks a capability and a medium none
/// of them does: one for every capability a variable asks the address of, by the medium it
/// files and the API it talks in, and one for the capability whose filler's network it runs
/// inside.
pub(crate) fn merged(
    mut asks: Vec<Asking>,
    env: &BTreeMap<String, Value>,
    network: Option<&Network>,
) -> Vec<Asking> {
    let addressed = env.values().filter_map(|value| match value {
        Value::Url(url) => Some(Asking {
            capability: url.url_of.clone(),
            each: false,
            native: url
                .native
                .as_deref()
                .map(lemonfiber_manifest::container::Apis::one),
            files: url.files.clone(),
        }),
        _ => None,
    });
    let inside = network.and_then(Network::via).map(|capability| Asking {
        capability: capability.to_owned(),
        each: false,
        native: None,
        files: None,
    });
    for implied in addressed.chain(inside) {
        match asks.iter_mut().find(|ask| ask.is_of(&implied)) {
            Some(ask) => {
                if ask.native.is_none() {
                    ask.native = implied.native;
                }
            }
            None => asks.push(implied),
        }
    }
    asks
}

/// One capability a placed service asks for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginAsking")]
pub struct Asking {
    /// The core capability asked for.
    pub capability: String,
    /// Whether every service that fills it is reached rather than one.
    #[serde(default)]
    pub each: bool,
    /// The upstream's own API it talks in, or every API it talks in any of, which it
    /// reaches in place of the adapter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native: Option<lemonfiber_manifest::container::Apis>,
    /// The medium the filler it reaches files, where it narrows the ask to one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<String>,
}

impl Asking {
    /// Whether `other` asks the capability and the medium this one asks.
    #[must_use]
    pub fn is_of(&self, other: &Self) -> bool {
        self.capability == other.capability && self.files == other.files
    }
}
