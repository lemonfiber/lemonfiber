//! What a plugin would take that this machine already holds.

use std::path::PathBuf;

use super::{answered, bound, named, occupied, unoccupied};
use crate::plugin::{Placed, Reached};
use crate::stack::declared::{Declared, Published};
use crate::test_support::{a_context, a_placed, an_installed};

/// A service published on `port`, proxied at `label` where one is given.
fn reached(service: &str, port: u16, label: Option<&str>) -> Placed {
    Placed {
        reached: Some(
            label.map_or(Reached::Loopback { port, group: None }, |hostname| {
                Reached::Household {
                    port,
                    hostname: hostname.to_owned(),
                    group: None,
                }
            }),
        ),
        ..a_placed(service, &[], None, None)
    }
}

/// A service the operator's overlay declares is refused by name, naming the file.
#[test]
fn a_service_the_overlay_declares_is_refused_naming_the_file() {
    let would = an_installed("tools", vec![a_placed("watchtower", &[], None, None)]);
    let held = [Declared {
        service: "watchtower".to_owned(),
        file: PathBuf::from("/srv/stack/compose.override.yml"),
    }];

    let said = named(&would, &held);

    assert_eq!(said.len(), 1);
    assert!(
        said.iter()
            .all(|one| one.contains("watchtower") && one.contains("compose.override.yml")),
        "{said:?}"
    );
    assert!(named(
        &an_installed("tools", vec![a_placed("other", &[], None, None)]),
        &held
    )
    .is_empty());
}

/// A port the stack publishes, and one another plugin publishes, are each refused; the
/// plugin's own earlier version is not another plugin.
#[test]
fn a_port_the_stack_or_another_plugin_publishes_is_refused() {
    let would = an_installed("tools", vec![reached("web", 443, Some("tools"))]);
    let stack = [Published {
        port: 443,
        service: "caddy".to_owned(),
        file: PathBuf::from("compose/proxy.yml"),
    }];
    let said = bound(&would, &stack, &[]);
    assert!(
        said.iter()
            .any(|one| one.contains("443") && one.contains("caddy")),
        "{said:?}"
    );

    let other = an_installed("other", vec![reached("dash", 443, None)]);
    let earlier = an_installed("tools", vec![reached("web", 443, None)]);
    let said = bound(&would, &[], &[other, earlier]);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said.iter().all(|one| one.contains("other's dash")),
        "{said:?}"
    );

    let free = an_installed("tools", vec![reached("web", 25600, None)]);
    assert!(bound(&free, &stack, &[]).is_empty());
}

/// A label a site in the live proxy configuration answers on is refused; the plugin's
/// own region in that file is not somebody else's site.
#[test]
fn a_label_the_live_proxy_answers_on_is_refused_and_its_own_region_is_not() {
    let would = an_installed("tools", vec![reached("web", 25600, Some("files"))]);
    let by_hand = "files.{$DOMAIN:home.local} {\n\treverse_proxy nas:80\n}\n";
    let said = answered(&would, by_hand);
    assert!(said.iter().any(|one| one.contains("files")), "{said:?}");

    let own = crate::region::put(
        "watch.{$DOMAIN:home.local} {\n}\n",
        &crate::plugin::owner("tools"),
        "files.{$DOMAIN:home.local} {\n\treverse_proxy web:25600\n}\n",
    );
    assert!(answered(&would, &own).is_empty(), "{own}");
}

/// Every clash is in the one refusal, with its code.
#[test]
fn every_clash_is_in_one_refusal() {
    let problem = occupied("tools", &["one".to_owned(), "two".to_owned()]);
    assert_eq!(problem.code.to_string(), "PLUGIN-28");
    assert_eq!(problem.detail.as_deref(), Some("one; two"));
}

/// Against the stack this build ships, the proxy's own ports are taken and a free one is
/// not.
#[test]
fn the_shipped_stack_holds_the_proxys_ports() {
    let ctx = a_context().build();
    let scratch = std::env::temp_dir().join(format!("lemonfiber-occupied-{}", std::process::id()));
    let taken = an_installed("tools", vec![reached("web", 443, None)]);
    let free = an_installed("tools", vec![reached("web", 25600, None)]);

    let refused = unoccupied(&ctx, &taken, &[], &scratch).err();

    assert_eq!(
        refused.as_ref().map(|one| one.code.to_string()).as_deref(),
        Some("PLUGIN-28")
    );
    assert!(refused
        .as_ref()
        .and_then(|one| one.detail.as_deref())
        .is_some_and(|detail| detail.contains("443")));
    assert!(unoccupied(&ctx, &free, &[], &scratch).is_ok());
}
