//! The proxy stanza and the dashboard entry a plugin's service gets.
//!
//! The bundled stack puts a household service behind its proxy and every service on
//! its dashboard. A plugin's service gets the same on the same terms, and gets it
//! written *for* it: what goes in each is derived here from what the install
//! recorded, which is what the manifest declared (the service's id and port, the tier
//! that decides whether it is reachable at all, and the description beside it). There
//! is no field a plugin could put a line of proxy or dashboard configuration in, so
//! nothing here reads one.
//!
//! **The tier decides the route and nothing else.** A household service gets a
//! stanza and an entry linking to the household address. An operator surface gets an
//! entry linking to this machine and no stanza, which is the bundled policy holding:
//! an admin surface does not get a name on the household network. The record cannot
//! carry a hostname for one in the first place, so there is nothing here to refuse.
//!
//! **A link, not a widget.** An entry is an icon, a link and a description. A widget
//! reads a service's API with a credential, which is a recipe's to capture, and this
//! build runs none.
//!
//! Nothing here touches a disk. It answers with text, and where that text goes, and
//! who owns the region it goes in, is [`super::placing`]'s.

use std::fmt::Write as _;

use super::installed::{Installed, Placed, Reached};

/// The proxy's configuration, beneath the stack directory.
pub const PROXY: &str = "config/caddy/Caddyfile";

/// The dashboard's list of services, beneath the stack directory.
pub const DASHBOARD: &str = "config/homepage/services.yaml";

/// The group a household service is listed under where its manifest named none.
///
/// The stack's own answer, read off the dashboard it ships: the group its household
/// library services are in.
const HOUSEHOLD_GROUP: &str = "Library";

/// The group an operator surface is listed under where its manifest named none: the
/// group the shipped dashboard keeps the stack's own automation in.
const OPERATOR_GROUP: &str = "Automation";

/// Whose region a plugin's wiring is written in, as the region's markers name it.
#[must_use]
pub fn owner(plugin: &str) -> String {
    format!("plugin {plugin}")
}

/// The proxy stanza for every one of the plugin's services the household reaches.
///
/// Empty where there is none, which is a plugin whose services are all operator
/// surfaces or reached by nothing, and which writes nothing into the proxy at all.
/// The same shape as the stack's own stanzas: the label in front of the operator's
/// domain, and the service reached by its name and port on the stack's network.
#[must_use]
pub fn proxied(installed: &Installed) -> String {
    installed
        .services
        .iter()
        .filter_map(|placed| match &placed.reached {
            Some(Reached::Household { port, hostname, .. }) => Some(format!(
                "{hostname}.{{$DOMAIN:home.local}} {{\n\treverse_proxy {}:{port}\n}}\n",
                placed.service
            )),
            Some(Reached::Loopback { .. }) | None => None,
        })
        .collect::<Vec<String>>()
        .join("\n")
}

/// A label one of this plugin's household services would answer on that a service of
/// another installed plugin already answers on, with whose it is.
///
/// Refused before anything is written rather than written and left to the proxy,
/// because the proxy does not pick one: two sites at one address is a configuration it
/// will not start with, and every household route would go down with it. The same
/// plugin's own record is not another plugin's, so an update replacing a version is
/// not held to the labels of the version it replaces.
#[must_use]
pub fn taken(would: &Installed, installed: &[Installed]) -> Option<(String, String)> {
    let answering = |plugin: &Installed| -> Vec<String> {
        plugin
            .services
            .iter()
            .filter_map(|placed| placed.reached.as_ref()?.hostname().map(str::to_owned))
            .collect()
    };
    let wanted = answering(would);
    installed
        .iter()
        .filter(|other| other.plugin != would.plugin)
        .find_map(|other| {
            answering(other)
                .into_iter()
                .find(|label| wanted.contains(label))
                .map(|label| (label, other.plugin.clone()))
        })
}

/// The dashboard entries for every one of the plugin's services that listens, under
/// the group each belongs to.
///
/// Empty where nothing listens. Grouped in the order the groups first appear, so the
/// entry is the same twice and a rewrite for no reason is no change at all. Every
/// name and every value is written as a quoted string, because a plugin's name or
/// description is prose from its author and a colon in it must not become YAML.
#[must_use]
pub fn listed(installed: &Installed) -> String {
    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    for placed in &installed.services {
        let Some(reached) = &placed.reached else {
            continue;
        };
        let group = reached
            .group()
            .map_or_else(|| default_group(reached).to_owned(), str::to_owned);
        let entry = entry(placed, reached);
        match groups.iter_mut().find(|(named, _)| *named == group) {
            Some((_, entries)) => entries.push(entry),
            None => groups.push((group, vec![entry])),
        }
    }
    groups
        .into_iter()
        .fold(String::new(), |mut text, (group, entries)| {
            let _ = write!(text, "- {}:\n{}", quoted(&group), entries.concat());
            text
        })
}

/// The group a service is listed under where its manifest named none.
const fn default_group(reached: &Reached) -> &'static str {
    match reached {
        Reached::Household { .. } => HOUSEHOLD_GROUP,
        Reached::Loopback { .. } => OPERATOR_GROUP,
    }
}

/// One service's entry: its icon, where the link goes, and what it is for.
///
/// The link is rendered from the tier exactly as the stack's own are: the household
/// address for a household service, and this machine for an operator surface, whose
/// port is published nowhere else.
fn entry(placed: &Placed, reached: &Reached) -> String {
    let href = match reached {
        Reached::Household { port, .. } => format!("http://{{{{HOMEPAGE_VAR_LAN_HOST}}}}:{port}"),
        Reached::Loopback { port, .. } => format!("http://localhost:{port}"),
    };
    let name = if placed.name.is_empty() {
        &placed.service
    } else {
        &placed.name
    };
    format!(
        "    - {}:\n        icon: {}\n        href: {}\n        description: {}\n",
        quoted(name),
        quoted(&format!("{}.png", placed.service)),
        quoted(&href),
        quoted(&placed.description),
    )
}

/// A string as YAML reads it back unchanged, whatever it holds.
///
/// A JSON string is a YAML double-quoted scalar, escaping included, so the one
/// serialiser that is already here says it without a second one.
fn quoted(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_default()
}

#[cfg(test)]
mod tests;
