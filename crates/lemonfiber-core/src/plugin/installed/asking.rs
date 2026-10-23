//! What a placed service asks for.

use lemonfiber_manifest::container::Value;
use lemonfiber_plugin::{Manifest, Service};
use serde::{Deserialize, Serialize};

/// Every capability a service asks for: each `[[ask]]` it makes, then each capability a
/// variable asks the address of, narrowed by no medium and no API, that no `[[ask]]`
/// names, once.
pub(super) fn asks(manifest: &Manifest, service: &Service) -> Vec<Asking> {
    let mut asks: Vec<Asking> = manifest
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
        })
        .collect();
    for value in service.env.values() {
        if let Value::Url(url) = value {
            let narrowed = url.files.is_some() || url.native.is_some();
            if !narrowed && !asks.iter().any(|ask| ask.capability == url.url_of) {
                asks.push(Asking {
                    capability: url.url_of.clone(),
                    each: false,
                    native: None,
                });
            }
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
    /// The upstream's own API it talks in, which it reaches in place of the adapter.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native: Option<String>,
}
