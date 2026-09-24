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
mod tests {
    use super::{listed, owner, proxied, taken, HOUSEHOLD_GROUP, OPERATOR_GROUP};
    use crate::plugin::installed::{Installed, Placed, Reached};

    fn placed(service: &str, reached: Option<Reached>) -> Placed {
        Placed {
            service: service.to_owned(),
            image: "example.invalid/komga".to_owned(),
            digest: "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
                .to_owned(),
            tag: "1.11.0".to_owned(),
            config_path: "/config".to_owned(),
            takes_data: false,
            reached,
            provides: Vec::new(),
            name: "Komga".to_owned(),
            description: "Reads comics: in a browser".to_owned(),
        }
    }

    fn installed(services: Vec<Placed>) -> Installed {
        Installed {
            plugin: "comics".to_owned(),
            version: "1.0.0".to_owned(),
            services,
            provides: Vec::new(),
            contributions: Vec::new(),
            declared: crate::plugin::Declaration::default(),
            from: String::new(),
            installed_at: String::new(),
        }
    }

    fn household(group: Option<&str>) -> Reached {
        Reached::Household {
            port: 25600,
            hostname: "comics".to_owned(),
            group: group.map(str::to_owned),
        }
    }

    const fn loopback() -> Reached {
        Reached::Loopback {
            port: 8090,
            group: None,
        }
    }

    #[test]
    fn a_household_service_is_proxied_at_its_label_by_its_name_and_port() {
        assert_eq!(
            proxied(&installed(vec![placed("komga", Some(household(None)))])),
            "comics.{$DOMAIN:home.local} {\n\treverse_proxy komga:25600\n}\n"
        );
    }

    /// The bundled policy holding: an operator surface is never given a route.
    #[test]
    fn an_operator_surface_and_a_service_nothing_reaches_are_not_proxied() {
        let plugin = installed(vec![
            placed("admin", Some(loopback())),
            placed("worker", None),
        ]);

        assert_eq!(proxied(&plugin), "");
    }

    #[test]
    fn two_household_services_are_two_stanzas() {
        let mut second = placed("reader", Some(household(None)));
        second.reached = Some(Reached::Household {
            port: 8080,
            hostname: "reader".to_owned(),
            group: None,
        });
        let both = proxied(&installed(vec![
            placed("komga", Some(household(None))),
            second,
        ]));

        assert_eq!(both.matches("reverse_proxy").count(), 2, "{both}");
    }

    #[test]
    fn a_household_service_is_listed_linking_to_the_household_address() {
        let said = listed(&installed(vec![placed("komga", Some(household(None)))]));

        assert_eq!(
            said,
            "- \"Library\":\n    - \"Komga\":\n        icon: \"komga.png\"\n        href: \
             \"http://{{HOMEPAGE_VAR_LAN_HOST}}:25600\"\n        description: \"Reads comics: in \
             a browser\"\n"
        );
    }

    #[test]
    fn an_operator_surface_is_listed_linking_to_this_machine_under_the_automation_group() {
        let said = listed(&installed(vec![placed("admin", Some(loopback()))]));

        assert!(said.starts_with("- \"Automation\":\n"), "{said}");
        assert!(said.contains("\"http://localhost:8090\""), "{said}");
    }

    #[test]
    fn a_group_the_manifest_named_is_the_one_used_and_shared_by_its_services() {
        let said = listed(&installed(vec![
            placed("komga", Some(household(Some("Reading")))),
            placed("admin", Some(loopback())),
            placed("second", Some(household(Some("Reading")))),
        ]));

        assert_eq!(said.matches("- \"Reading\":").count(), 1, "{said}");
        assert_eq!(said.matches("- \"Automation\":").count(), 1, "{said}");
    }

    #[test]
    fn a_service_nothing_reaches_is_not_listed_and_one_with_no_name_is_listed_by_id() {
        let mut nameless = placed("komga", Some(household(None)));
        nameless.name = String::new();
        let said = listed(&installed(vec![placed("worker", None), nameless]));

        assert!(!said.contains("worker"), "{said}");
        assert!(said.contains("- \"komga\":"), "{said}");
    }

    /// The defaults are the stack's answer rather than this module's, so they are held
    /// to the dashboard the stack ships: each is a group there, holding the tier it is
    /// the default for.
    #[test]
    fn each_default_group_is_one_the_shipped_dashboard_keeps_that_tier_in() {
        let shipped = include_str!("../../../../assets/media-stack/config/homepage/services.yaml");
        for (group, link) in [
            (HOUSEHOLD_GROUP, "http://{{HOMEPAGE_VAR_LAN_HOST}}"),
            (OPERATOR_GROUP, "http://localhost"),
        ] {
            let after = shipped
                .split(&format!("- {group}:\n"))
                .nth(1)
                .and_then(|rest| rest.split("\n- ").next())
                .unwrap_or_default();
            assert!(after.contains(link), "{group} holds no {link} entry");
        }
    }

    #[test]
    fn a_label_another_plugin_answers_on_is_taken_and_one_nobody_does_is_not() {
        let mut other = installed(vec![placed("komga", Some(household(None)))]);
        other.plugin = "library".to_owned();
        let mut free = installed(vec![placed("komga", Some(household(None)))]);
        free.services = vec![placed("reader", Some(loopback()))];

        let wanted = installed(vec![placed("komga", Some(household(None)))]);

        assert_eq!(
            taken(&wanted, std::slice::from_ref(&other)),
            Some(("comics".to_owned(), "library".to_owned()))
        );
        assert_eq!(
            taken(&free, &[other]),
            None,
            "an operator surface answers on no label"
        );
        assert_eq!(
            taken(&wanted, std::slice::from_ref(&wanted)),
            None,
            "the version being replaced is not another plugin"
        );
    }

    #[test]
    fn a_plugins_region_is_named_as_the_plugins() {
        assert_eq!(owner("comics"), "plugin comics");
    }
}
