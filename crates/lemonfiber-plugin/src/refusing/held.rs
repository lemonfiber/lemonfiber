//! What every part the bundle pins holds, each by the plugin holding it.
//!
//! The ids and published ports of every service a pinned part declares, `assembled` and
//! `installed` alike, beside the stack's own, and every name the household reaches a
//! part at, the ones the proxy's configuration in its copy answers on among them. Each is
//! held by a plugin's `[plugin].id`, so a refusal names the plugin, and the plugin of
//! that id is the one manifest none of it is refused to.

use std::sync::LazyLock;

use lemonfiber_contract::capabilities::proxy::front;
use lemonfiber_manifest::container::published;
use serde_json::Value;

use super::bundled;

/// Where a part's copy keeps its services' configuration.
const CONFIGURED: &str = "config/";

/// The kind of file the proxy's configuration is written in.
const CONFIGURATION: &str = ".json";

/// A service a part the bundle pins holds, by the plugin holding it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Held {
    /// The `[plugin].id` of the part whose pin holds it.
    pub(super) plugin: String,
    /// The service's id.
    pub(super) id: String,
    /// The service's name.
    pub(super) name: String,
    /// Every port it publishes.
    pub(super) ports: Vec<u16>,
}

/// A name the household reaches a part the bundle pins at, by the plugin holding it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Answered {
    /// The label, the first of the name's dot-separated parts.
    pub(super) label: String,
    /// The `[plugin].id` of the part answering on it.
    pub(super) plugin: String,
}

/// Every service the parts the bundle pins hold, read once from what this build embeds.
pub(super) fn held() -> &'static [Held] {
    static HELD: LazyLock<Vec<Held>> = LazyLock::new(|| reserved(&copies(), bundled::services()));
    &HELD
}

/// Every name the household reaches a part the bundle pins at: each part's own labels,
/// then each label the proxy's configuration answers on that no part declares, held by
/// the proxy's plugin.
pub(super) fn answered() -> &'static [Answered] {
    static ANSWERED: LazyLock<Vec<Answered>> = LazyLock::new(|| {
        lemonfiber_bundled::PINNED
            .iter()
            .fold(labels(&copies()), |answered, pinned| {
                joined(
                    answered,
                    pinned.first_party.plugin,
                    proxied(pinned.fills, pinned.files),
                )
            })
    });
    &ANSWERED
}

/// The service a plugin other than `own` holds under `id`.
pub(super) fn taking<'a>(held: &'a [Held], own: &str, id: &str) -> Option<&'a Held> {
    held.iter().find(|one| one.plugin != own && one.id == id)
}

/// The service a plugin other than `own` holds under an id written as `id` is once each
/// is spelled as an environment name.
pub(super) fn spelled_as<'a>(held: &'a [Held], own: &str, id: &str) -> Option<&'a Held> {
    let spelled = lemonfiber_manifest::environment_name(id);
    held.iter()
        .find(|one| one.plugin != own && lemonfiber_manifest::environment_name(&one.id) == spelled)
}

/// The service a plugin other than `own` publishes on `port`.
pub(super) fn on_port<'a>(held: &'a [Held], own: &str, port: u16) -> Option<&'a Held> {
    held.iter()
        .find(|one| one.plugin != own && one.ports.contains(&port))
}

/// The plugin other than `own` the household reaches at `label`.
pub(super) fn answering<'a>(answered: &'a [Answered], own: &str, label: &str) -> Option<&'a str> {
    answered
        .iter()
        .find(|one| one.plugin != own && one.label == label)
        .map(|one| one.plugin.as_str())
}

/// `answered`, with each of `labels` no part already answers on held by `plugin`.
fn joined(mut answered: Vec<Answered>, plugin: &str, labels: Vec<String>) -> Vec<Answered> {
    for label in labels {
        if !answered.iter().any(|one| one.label == label) {
            answered.push(Answered {
                label,
                plugin: plugin.to_owned(),
            });
        }
    }
    answered
}

/// Every pinned part's manifest, by its plugin's id.
fn copies() -> Vec<(&'static str, crate::Manifest)> {
    lemonfiber_bundled::PINNED
        .iter()
        .filter_map(|pinned| {
            crate::Manifest::from_toml(pinned.copy)
                .ok()
                .map(|part| (pinned.first_party.plugin, part))
        })
        .collect()
}

/// Every service `parts` declare, then every one `stack` declares, each held by the part
/// declaring a service of its id, or by the plugin of its own id where none does.
fn reserved(
    parts: &[(&str, crate::Manifest)],
    stack: &[lemonfiber_manifest::Service],
) -> Vec<Held> {
    let mut held: Vec<Held> = parts
        .iter()
        .flat_map(|(plugin, part)| {
            part.services.iter().map(move |service| Held {
                plugin: (*plugin).to_owned(),
                id: service.id.clone(),
                name: service.name.clone(),
                ports: published(service.port, &service.publish).collect(),
            })
        })
        .collect();
    let stacked: Vec<Held> = stack
        .iter()
        .map(|service| Held {
            plugin: held
                .iter()
                .find(|one| one.id == service.id)
                .map_or_else(|| service.id.clone(), |one| one.plugin.clone()),
            id: service.id.clone(),
            name: service.name.clone(),
            ports: published(service.port, &service.publish).collect(),
        })
        .collect();
    held.extend(stacked);
    held
}

/// The label each household service of `parts` is proxied at, by its plugin.
fn labels(parts: &[(&str, crate::Manifest)]) -> Vec<Answered> {
    parts
        .iter()
        .flat_map(|(plugin, part)| {
            part.services
                .iter()
                .filter(|service| service.port.is_some() && service.bind == Some(crate::Bind::Lan))
                .map(move |service| Answered {
                    label: part.entry(service).hostname.to_owned(),
                    plugin: (*plugin).to_owned(),
                })
        })
        .collect()
}

/// Every label the proxy's configuration among `carried` answers on, where `fills` holds
/// the proxy's capability.
fn proxied(fills: &[&str], carried: &[(&str, &[u8])]) -> Vec<String> {
    if !fills.contains(&front::CAPABILITY) {
        return Vec::new();
    }
    let mut found = Vec::new();
    for (path, bytes) in carried {
        if !path.starts_with(CONFIGURED) || !path.ends_with(CONFIGURATION) {
            continue;
        }
        if let Ok(read) = serde_json::from_slice::<Value>(bytes) {
            matched(&read, &mut found);
        }
    }
    found.sort();
    found.dedup();
    found
}

/// Every label a route of `value` matches a request's host on, at any depth.
fn matched(value: &Value, found: &mut Vec<String>) {
    match value {
        Value::Object(fields) => {
            for (field, inner) in fields {
                if field == "match" {
                    hosts(inner, found);
                }
                matched(inner, found);
            }
        }
        Value::Array(items) => {
            for item in items {
                matched(item, found);
            }
        }
        _ => {}
    }
}

/// The label of each host a list of matchers names.
fn hosts(matchers: &Value, found: &mut Vec<String>) {
    let named = matchers
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|matcher| matcher.get("host"))
        .filter_map(Value::as_array)
        .flatten()
        .filter_map(Value::as_str);
    for host in named {
        if let Some(label) = host.split('.').next().filter(|label| !label.is_empty()) {
            found.push(label.to_owned());
        }
    }
}

#[cfg(test)]
mod tests;
