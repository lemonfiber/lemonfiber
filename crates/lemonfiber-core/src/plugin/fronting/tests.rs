use super::{proxied, taken};
use crate::plugin::installed::{Installed, Placed, Reached};

fn placed(service: &str, reached: Option<Reached>) -> Placed {
    Placed {
        fields: lemonfiber_manifest::container::Fields::default(),
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
        icon: None,
        hidden: false,
        health: None,
        api: None,
        listens: None,
        media_types: Vec::new(),
        networks: Vec::new(),
        speaks: Vec::new(),
        fronts: None,
        upstream: std::collections::BTreeMap::new(),
        native: None,
        replaces: None,
        shape: None,
        asks: Vec::new(),
        reaches: None,
        asks_for: None,
        starting: None,
    }
}

fn installed(services: Vec<Placed>) -> Installed {
    Installed {
        plugin: "comics".to_owned(),
        version: "1.0.0".to_owned(),
        name: None,
        description: None,
        services,
        provides: Vec::new(),
        contributions: Vec::new(),
        run_proofs: Vec::new(),
        request_proofs: Vec::new(),
        declared: crate::plugin::Declaration::default(),
        recipes: Vec::new(),
        adapters: Vec::new(),
        from: String::new(),
        revision: String::new(),
        signed: String::new(),
        manifest: String::new(),
        tree: String::new(),
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
