use super::{routes, taken};
use crate::plugin::installed::{Installed, Placed, Reached};
use crate::ports::service::Route;

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
        listens: None,
        contract: None,
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
        granted: Vec::new(),
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

/// A route for `host` at `port`, answering `label` under the test's domain.
fn route(label: &str, host: &str, port: u16) -> Route {
    Route {
        name: format!("{label}.home.example"),
        host: host.to_owned(),
        port,
    }
}

#[test]
fn a_household_service_is_routed_at_its_label_to_its_name_and_the_port_it_listens_on() {
    let mut listening = placed("komga", Some(household(None)));
    assert_eq!(
        routes(&[installed(vec![listening.clone()])], "home.example"),
        vec![route("comics", "komga", 25600)]
    );
    listening.listens = Some(8080);
    assert_eq!(
        routes(&[installed(vec![listening])], "home.example"),
        vec![route("comics", "komga", 8080)]
    );
}

/// The bundled policy holding: an operator surface is never given a route.
#[test]
fn an_operator_surface_and_a_service_nothing_reaches_are_not_routed() {
    let plugin = installed(vec![
        placed("admin", Some(loopback())),
        placed("worker", None),
    ]);

    assert!(routes(&[plugin], "home.example").is_empty());
}

#[test]
fn every_household_service_of_every_plugin_is_a_route_in_the_order_installed() {
    let mut second = placed("reader", Some(household(None)));
    second.reached = Some(Reached::Household {
        port: 8080,
        hostname: "reader".to_owned(),
        group: None,
    });
    let mut other = installed(vec![second]);
    other.plugin = "reading".to_owned();

    assert_eq!(
        routes(
            &[
                installed(vec![placed("komga", Some(household(None)))]),
                other
            ],
            "home.example"
        ),
        vec![
            route("comics", "komga", 25600),
            route("reader", "reader", 8080)
        ]
    );
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
fn no_route_is_made_to_the_port_a_service_speaks_its_contracts_on() {
    let routed = |contract: u16| {
        let mut door = placed(
            "door",
            Some(Reached::Household {
                port: 8081,
                hostname: "watch".to_owned(),
                group: None,
            }),
        );
        door.contract = Some(contract);
        door.speaks = vec!["stream.guard@1".to_owned()];
        routes(&[installed(vec![door])], "home.example")
    };
    assert!(routed(8081).is_empty());
    assert_eq!(routed(8082), vec![route("watch", "door", 8081)]);
}

#[test]
fn a_stack_household_service_is_routed_at_the_label_its_part_declares_until_it_moves() {
    use lemonfiber_manifest::Bind;
    let mut requests = crate::door::fixtures::service("seerr", Some(Bind::Lan));
    requests.port = Some(5055);
    let wanted: Vec<Route> = lemonfiber_plugin::refusing::shipped_label("seerr")
        .map(|label| Route {
            name: format!("{label}.home.example"),
            host: "seerr".to_owned(),
            port: 5055,
        })
        .into_iter()
        .collect();
    assert!(
        !wanted.is_empty(),
        "the bundle declares the request service's label"
    );
    let mut portless = requests.clone();
    portless.port = None;
    let services = [
        requests.clone(),
        portless,
        crate::door::fixtures::service("sonarr", Some(Bind::Loopback)),
        crate::door::fixtures::service("stranger", Some(Bind::Lan)),
    ];

    assert_eq!(super::stack_routes(&services, &[], "home.example"), wanted);
    let moved = crate::test_support::adapted::installed_parts();
    assert!(super::stack_routes(&[requests], &moved, "home.example").is_empty());
}
