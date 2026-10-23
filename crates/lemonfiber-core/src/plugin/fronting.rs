//! The proxy stanza a plugin's service gets.
//!
//! The bundled stack puts a household service behind its proxy. A plugin's service gets
//! the same on the same terms, and gets it written *for* it: what goes in the stanza is
//! derived here from what the install recorded, which is what the manifest declared (the
//! service's id and port, and the tier that decides whether it is reachable at all).
//! There is no field a plugin could put a line of proxy configuration in, so nothing here
//! reads one. Its dashboard entry is the panel list's, applied to the dashboard by
//! [`crate::app::showing`].
//!
//! **The tier decides the route.** A household service gets a stanza; an operator
//! surface gets none, which is the bundled policy holding: an admin surface does not get
//! a name on the household network. The record cannot carry a hostname for one in the
//! first place, so there is nothing here to refuse.
//!
//! Nothing here touches a disk. It answers with text, and where that text goes, and
//! who owns the region it goes in, is [`super::placing`]'s.

use super::installed::{Installed, Reached};

/// The proxy's configuration, beneath the stack directory.
pub const PROXY: &str = "config/caddy/Caddyfile";

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

#[cfg(test)]
mod tests;
