//! The routes the household's proxy serves for the installed plugins' services.
//!
//! The stack's own household service gets a route at the label the bundle's part declares
//! for it, and a plugin's on the same terms, derived here from what the install recorded: the label in front of the operator's domain, and
//! the service reached by its name and the port it listens on inside its container.
//! There is no field a plugin could put a line of proxy configuration in, so nothing here
//! reads one.
//!
//! **The tier decides the route.** A household service gets one; an operator surface
//! gets none, which is the bundled policy holding: an admin surface does not get a name
//! on the household network. The record cannot carry a hostname for one in the first
//! place, so there is nothing here to refuse. Nor is a route ever made to the port a
//! service speaks its contracts on.

use lemonfiber_manifest::container::{exposes, inside, Reach};
use lemonfiber_manifest::{Bind, Service};

use super::installed::{Installed, Reached};
use crate::ports::service::Route;

/// A route for every household service of the stack's own `services` that none of
/// `installed` moved in place of, at the label the bundle's part declares for it in front
/// of `domain`; none for one no part declares a label for.
#[must_use]
pub fn stack_routes(services: &[Service], installed: &[Installed], domain: &str) -> Vec<Route> {
    services
        .iter()
        .filter(|service| service.bind == Some(Bind::Lan))
        .filter(|service| !crate::project::moved_in_place(installed, &service.id))
        .filter_map(|service| {
            let label = lemonfiber_plugin::refusing::shipped_label(&service.id)?;
            Some(Route {
                name: format!("{label}.{domain}"),
                host: service.id.clone(),
                port: inside(service.port?, service.listens),
            })
        })
        .collect()
}

/// A route for every household service of every one of `installed`, at its label in
/// front of `domain`, in the order they are installed.
#[must_use]
pub fn routes(installed: &[Installed], domain: &str) -> Vec<Route> {
    installed
        .iter()
        .flat_map(|plugin| &plugin.services)
        .filter_map(|placed| {
            let Some(Reached::Household { port, hostname, .. }) = &placed.reached else {
                return None;
            };
            let at = inside(*port, placed.listens);
            (!exposes(Reach::Route, at, placed.contract_port(), placed.listens)).then(|| Route {
                name: format!("{hostname}.{domain}"),
                host: placed.service.clone(),
                port: at,
            })
        })
        .collect()
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
