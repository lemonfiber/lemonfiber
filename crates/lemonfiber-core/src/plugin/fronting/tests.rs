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
    let shipped = include_str!("../../../../../assets/media-stack/config/homepage/services.yaml");
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
