use lemonfiber_core::config::{Reaching, Settings};
use lemonfiber_core::origin::Origin;
use lemonfiber_core::outbound::{leaving as gathered, Elsewhere, Leaving};

use super::leaving;

/// The report as the core builds it for a machine with no stack materialised —
/// which is every entry lemonfiber makes on its own account, and nothing of the
/// stack's, since what the services reach is read from the services there are.
fn shown(settings: &Settings) -> String {
    leaving(&gathered(settings, &[], &[])).text()
}

#[test]
fn every_request_is_named_with_where_it_goes_and_what_it_sends() {
    let said = shown(&Settings::default());
    for named in ["registry", "guides", "echo", "indexer", "usenet"] {
        assert!(said.contains(named), "{named} is not on the screen: {said}");
    }
    assert!(said.contains("sends"), "{said}");
    assert!(said.contains("costs"), "{said}");
}

#[test]
fn a_request_that_is_on_says_so_and_one_that_is_off_says_so() {
    let on = shown(&Settings::default());
    assert!(on.contains("registry — on"), "{on}");
    let off = shown(&Settings {
        ip_echo: Vec::new(),
        reaching: Reaching::none(),
        ..Settings::default()
    });
    assert!(off.contains("registry — off"), "{off}");
    assert!(off.contains("echo — off"), "{off}");
}

#[test]
fn a_request_with_nowhere_to_go_says_that_rather_than_leaving_the_line_blank() {
    let said = shown(&Settings::default());
    assert!(said.contains("nothing configured"), "{said}");
}

#[test]
fn the_setting_that_stops_everything_is_offered_once_under_the_list() {
    let said = shown(&Settings::default());
    assert_eq!(said.matches("LEMONFIBER_OFFLINE").count(), 1, "{said}");
}

#[test]
fn each_request_is_offered_the_command_that_switches_it_off() {
    let said = shown(&Settings::default());
    for key in [
        "LEMONFIBER_REACH_REGISTRY",
        "LEMONFIBER_REACH_GUIDES",
        "LEMONFIBER_IP_ECHO",
        "LEMONFIBER_REACH_INDEXER",
        "LEMONFIBER_REACH_USENET",
    ] {
        assert!(
            said.contains(&format!("lemonfiber config set {key} off")),
            "{key} is listed with no way to switch it off: {said}"
        );
    }
}

#[test]
fn the_stacks_own_requests_are_headed_as_the_stacks() {
    let said = leaving(&Leaving {
        ours: Vec::new(),
        theirs: vec![Elsewhere {
            service: "prowlarr".to_owned(),
            destination: "the indexers you configured".to_owned(),
            purpose: "Runs the searches everything else asks for.".to_owned(),
            recorded: true,
            origin: Origin::Bundled,
        }],
    })
    .text();
    assert!(said.contains("theirs and not lemonfiber's"), "{said}");
    assert!(
        said.contains("prowlarr — the indexers you configured"),
        "{said}"
    );
}

#[test]
fn a_plugins_request_is_marked_with_the_plugin_that_brought_it() {
    let said = leaving(&Leaving {
        ours: Vec::new(),
        theirs: vec![Elsewhere {
            service: "komga".to_owned(),
            destination: "metadata.example".to_owned(),
            purpose: "A destination this plugin's recipes declare.".to_owned(),
            recorded: true,
            origin: Origin::Plugin {
                named: "comics".to_owned(),
            },
        }],
    })
    .text();
    assert!(said.contains("komga (plugin comics) — "), "{said}");
}

#[test]
fn a_service_that_reaches_nothing_says_nothing_leaves_rather_than_nowhere() {
    let said = leaving(&Leaving {
        ours: Vec::new(),
        theirs: vec![Elsewhere {
            service: "unpackerr".to_owned(),
            destination: String::new(),
            purpose: "Nothing. It extracts what it finds on this machine.".to_owned(),
            recorded: true,
            origin: Origin::Bundled,
        }],
    })
    .text();
    assert!(
        said.contains("unpackerr — nothing leaves this machine"),
        "{said}"
    );
}

/// Through the dispatcher rather than by calling this module, because what the
/// terminal draws is what the printer chose for the outcome — and an arm nothing
/// reaches renders nowhere however good the renderer under it is.
#[test]
fn the_printer_reaches_this_renderer_for_this_outcome() {
    let report = gathered(&Settings::default(), &[], &[]);
    let drawn = crate::render::shaped(&lemonfiber_core::app::Outcome::Outbound(report)).text();
    assert!(drawn.contains("What lemonfiber sends"), "{drawn}");
    assert!(drawn.contains("registry"), "{drawn}");
}

/// The line an unknown service gets, which must not be the one that promises
/// nothing leaves the machine.
#[test]
fn a_service_lemonfiber_has_no_record_for_says_so_rather_than_promising_anything() {
    let said = leaving(&Leaving {
        ours: Vec::new(),
        theirs: vec![Elsewhere {
            service: "somebodys-own-service".to_owned(),
            destination: "not known to lemonfiber".to_owned(),
            purpose: "This service is not one lemonfiber knows.".to_owned(),
            recorded: false,
            origin: Origin::Bundled,
        }],
    })
    .text();
    assert!(said.contains("somebodys-own-service"), "{said}");
    assert!(said.contains("not one lemonfiber knows"), "{said}");
    assert!(
        !said.contains("nothing leaves this machine"),
        "an unknown service was given the promise that belongs to a known one: {said}"
    );
}

#[test]
fn a_stack_with_no_services_says_nothing_about_them_at_all() {
    let said = leaving(&Leaving {
        ours: Vec::new(),
        theirs: Vec::new(),
    })
    .text();
    assert!(!said.contains("theirs"), "{said}");
}
