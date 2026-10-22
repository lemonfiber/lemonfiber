use lemonfiber_manifest::{Manifest, Service};
use lemonfiber_ports::service::{Entry, PageWidget, Panels};

use super::{panels, HOUSEHOLD_GROUP, OPERATOR_GROUP};
use crate::door::Place;
use crate::platform::Environment;
use crate::plugin::{Installed, Reached};
use crate::wiring::{Chosen, Fillers};

/// Where the search box sends a search in these cases.
const SEARCH: &str = "https://search.example/?q=";

/// The address the operator recorded for the household's links.
const HOUSEHOLD: &str = "192.168.1.20";

/// The stack this build carries.
fn carried() -> Vec<Service> {
    Manifest::from_toml(lemonfiber_bundled::STACK)
        .map(|manifest| manifest.services)
        .unwrap_or_default()
}

/// A machine whose household address was recorded and which offers no name of its own.
const fn place() -> Place<'static> {
    Place {
        named: None,
        recorded: Some(HOUSEHOLD),
        domain: None,
        environment: Environment::LinuxNative,
    }
}

/// The panel list for `services` and `installed`.
fn listed(services: &[Service], installed: &[Installed]) -> Panels {
    let manifest = Manifest::from_toml(lemonfiber_bundled::STACK).map(|mut manifest| {
        manifest.services = services.to_vec();
        manifest
    });
    let fillers = manifest
        .map(|manifest| Fillers::of(&manifest, installed, &Chosen::default(), None))
        .unwrap_or_default();
    panels(services, installed, &fillers, &place(), SEARCH)
}

/// Every entry, by its name.
fn entries(panels: &Panels) -> Vec<&Entry> {
    panels
        .groups
        .iter()
        .flat_map(|group| &group.entries)
        .collect()
}

/// The entry named `name`.
fn entry<'a>(panels: &'a Panels, name: &str) -> Option<&'a Entry> {
    entries(panels).into_iter().find(|one| one.name == name)
}

/// The group holding the entry named `name`.
fn group_of<'a>(panels: &'a Panels, name: &str) -> Option<&'a str> {
    panels
        .groups
        .iter()
        .find(|group| group.entries.iter().any(|one| one.name == name))
        .map(|group| group.name.as_str())
}

/// `services` with the one whose id is `id` changed by `change`.
fn changed(
    mut services: Vec<Service>,
    id: &str,
    change: impl FnOnce(&mut Service),
) -> Vec<Service> {
    if let Some(one) = services.iter_mut().find(|one| one.id == id) {
        change(one);
    }
    services
}

#[test]
fn the_page_shows_the_machines_resources_and_a_search_box_sending_where_it_is_told() {
    assert_eq!(
        listed(&carried(), &[]).widgets,
        vec![
            PageWidget::Resources,
            PageWidget::Search {
                url: SEARCH.to_owned()
            },
        ]
    );
}

#[test]
fn every_service_with_a_port_has_an_entry_and_one_with_none_has_none() {
    let services = carried();
    let shown = listed(&services, &[]);
    for service in &services {
        let listed = entries(&shown).iter().any(|one| one.name == service.name);
        if service.port.is_none() {
            assert!(!listed, "{} publishes no port and is listed", service.id);
        }
    }
    assert!(!entries(&shown).is_empty());
}

#[test]
fn a_household_service_links_to_the_household_address_and_an_operator_surface_to_this_machine() {
    let services = changed(carried(), "sonarr", |one| {
        one.port = Some(8989);
        one.bind = Some(lemonfiber_manifest::Bind::Loopback);
    });
    let services = changed(services, "seerr", |one| {
        one.port = Some(5055);
        one.bind = Some(lemonfiber_manifest::Bind::Lan);
    });
    let shown = listed(&services, &[]);
    let link = |id: &str| {
        let name = services
            .iter()
            .find(|one| one.id == id)
            .map(|one| one.name.as_str());
        name.and_then(|name| entry(&shown, name))
            .map(|one| one.link.as_str())
    };

    assert_eq!(link("sonarr"), Some("http://localhost:8989"));
    assert_eq!(link("seerr"), Some("http://192.168.1.20:5055"));
}

#[test]
fn an_entry_shows_what_its_wiring_names_and_otherwise_the_services_own() {
    let services = changed(carried(), "sonarr", |one| {
        one.dashboard.group = Some("Watching".to_owned());
        one.dashboard.description = Some("Television".to_owned());
        one.dashboard.icon = Some("mdi-television".to_owned());
    });
    let named = |id: &str| services.iter().find(|one| one.id == id).cloned();
    let shown = listed(&services, &[]);

    let sonarr = named("sonarr").and_then(|one| entry(&shown, &one.name).cloned());
    assert_eq!(
        sonarr.as_ref().map(|one| one.description.as_str()),
        Some("Television")
    );
    assert_eq!(
        sonarr.as_ref().map(|one| one.icon.as_str()),
        Some("mdi-television")
    );
    let radarr = named("radarr");
    let shown_radarr = radarr.as_ref().and_then(|one| entry(&shown, &one.name));
    assert_eq!(
        shown_radarr.map(|one| one.description.as_str()),
        radarr.as_ref().map(|one| one.describes.as_str())
    );
    assert_eq!(
        shown_radarr.map(|one| one.icon.as_str()),
        Some("radarr.png")
    );
    let sonarr_name = named("sonarr").map(|one| one.name).unwrap_or_default();
    assert_eq!(group_of(&shown, &sonarr_name), Some("Watching"));
}

#[test]
fn a_service_its_wiring_hides_has_no_entry() {
    let services = changed(carried(), "sonarr", |one| one.dashboard.hidden = true);
    let name = services
        .iter()
        .find(|one| one.id == "sonarr")
        .map(|one| one.name.clone())
        .unwrap_or_default();

    assert!(entry(&listed(&services, &[]), &name).is_none());
    assert!(entry(&listed(&carried(), &[]), &name).is_some());
}

#[test]
fn a_service_naming_no_group_is_listed_under_its_tiers() {
    let services = changed(carried(), "sonarr", |one| one.dashboard.group = None);
    let services = changed(services, "seerr", |one| {
        one.dashboard.group = None;
        one.bind = Some(lemonfiber_manifest::Bind::Lan);
    });
    let shown = listed(&services, &[]);
    let name = |id: &str| {
        services
            .iter()
            .find(|one| one.id == id)
            .map(|one| one.name.clone())
            .unwrap_or_default()
    };

    assert_eq!(group_of(&shown, &name("sonarr")), Some(OPERATOR_GROUP));
    assert_eq!(group_of(&shown, &name("seerr")), Some(HOUSEHOLD_GROUP));
}

#[test]
fn the_media_server_links_through_its_guard_and_the_guard_has_no_entry_of_its_own() {
    let services = carried();
    let shown = listed(&services, &[]);
    let guard = services
        .iter()
        .find(|one| one.provides.iter().any(|one| one == "stream.guard"));
    let Some(guard) = guard else {
        unreachable!("the carried stack runs a stream guard");
    };

    assert!(entry(&shown, &guard.name).is_none(), "the guard is listed");
    let media = entries(&shown).into_iter().find(|one| {
        guard
            .port
            .is_some_and(|port| one.link.ends_with(&format!(":{port}")))
    });
    assert!(
        media.is_some(),
        "nothing links through the guard: {shown:?}"
    );
}

#[test]
fn a_service_answering_its_health_over_http_is_checked_where_the_stack_reaches_it() {
    let services = changed(carried(), "sonarr", |one| {
        one.health = Some(lemonfiber_manifest::Health {
            kind: lemonfiber_manifest::HealthKind::Http,
            path: Some("/ping".to_owned()),
            timeout_s: None,
        });
    });
    let services = changed(services, "radarr", |one| {
        one.health = Some(lemonfiber_manifest::Health {
            kind: lemonfiber_manifest::HealthKind::Tcp,
            path: None,
            timeout_s: None,
        });
    });
    let shown = listed(&services, &[]);
    let check = |id: &str| {
        services
            .iter()
            .find(|one| one.id == id)
            .and_then(|one| entry(&shown, &one.name))
            .map(|one| one.check.clone())
    };

    assert_eq!(
        check("sonarr"),
        Some(Some("http://sonarr:8989/ping".to_owned()))
    );
    assert_eq!(check("radarr"), Some(None));
}

#[test]
fn an_installed_plugins_service_is_listed_in_place_of_the_stack_service_it_moved_into() {
    let mut placed = crate::test_support::a_placed("sonarr", &[], None, Some(8989));
    placed.name = "Sonarr, moved".to_owned();
    placed.reached = Some(Reached::Loopback {
        port: 8989,
        group: Some("Moved".to_owned()),
    });
    let installed = crate::test_support::an_installed("sonarr", vec![placed]);
    let shown = listed(&carried(), std::slice::from_ref(&installed));

    assert_eq!(group_of(&shown, "Sonarr, moved"), Some("Moved"));
    assert!(
        entry(&shown, "Sonarr").is_none(),
        "the stack's own is listed too"
    );
}

/// The defaults are the stack's answer rather than this module's, so they are held to the
/// dashboard the stack ships: each is a group there, holding the tier it is the default
/// for.
#[test]
fn each_default_group_is_one_the_shipped_dashboard_keeps_that_tier_in() {
    let shipped = include_str!(
        "../../../../../../assets/plugins/bundle/plugins/homepage/config/homepage/services.yaml"
    );
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
