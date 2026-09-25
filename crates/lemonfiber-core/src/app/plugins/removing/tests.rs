use super::unfilled;
use crate::plugin::Installed;

/// A plugin filling exactly these capabilities, with nothing else recorded about
/// it that this rule reads.
fn filling(plugin: &str, provides: &[&str]) -> Installed {
    Installed {
        plugin: plugin.to_owned(),
        version: "1.0.0".to_owned(),
        services: Vec::new(),
        provides: provides.iter().map(|one| (*one).to_owned()).collect(),
        contributions: Vec::new(),
        declared: crate::plugin::Declaration::default(),
        from: String::new(),
        installed_at: String::new(),
    }
}

/// A stack service filling exactly these.
fn bundled(id: &str, provides: &[&str]) -> lemonfiber_manifest::Service {
    lemonfiber_manifest::Service {
        id: id.to_owned(),
        name: id.to_owned(),
        profile: "media".to_owned(),
        image: "example/image".to_owned(),
        tag: "1".to_owned(),
        port: None,
        bind: None,
        health: None,
        api: None,
        criticality: lemonfiber_manifest::Criticality::Core,
        license: "MIT".to_owned(),
        upstream: "https://example.test".to_owned(),
        last_release: "2026-01-01".to_owned(),
        describes: "an example service".to_owned(),
        without_it: "nothing works".to_owned(),
        media_types: Vec::new(),
        provides: provides.iter().map(|one| (*one).to_owned()).collect(),
        depends_on: Vec::new(),
        grants: Vec::new(),
        host_managed: false,
        memory_mib: None,
        asks_for: None,
        reaches: None,
    }
}

/// What a removal names, in order.
fn named(
    going: &Installed,
    held: &[Installed],
    stack: &[lemonfiber_manifest::Service],
) -> Vec<String> {
    unfilled(going, held, stack)
        .into_iter()
        .map(|one| one.capability)
        .collect()
}

/// The whole of the rule: a capability only this plugin fills is named, and one
/// something else still fills is not. An operator warned about the second would be
/// warned about something that is still there, which is how a warning stops being
/// read.
#[test]
fn only_a_capability_nothing_else_fills_is_named() {
    let going = filling("komga", &["media.serve", "library.curate"]);
    let held = vec![going.clone(), filling("kavita", &["library.curate"])];

    assert_eq!(
        named(&going, &held, &[]),
        vec!["media.serve"],
        "the one the other plugin does not also fill"
    );
}

/// The stack's own services count, and on a stock machine they are why this list is
/// almost always empty: every core capability the vocabulary carries is filled by
/// something lemonfiber ships, so a plugin that claims one was a second claimant
/// and its leaving is a contest resolving rather than a capability going.
#[test]
fn a_capability_the_bundled_stack_fills_is_not_named() {
    let going = filling("komga", &["media.serve"]);
    let held = vec![going.clone()];

    assert!(named(&going, &held, &[bundled("jellyfin", &["media.serve"])]).is_empty());
    assert_eq!(
        named(&going, &held, &[bundled("sonarr", &["library.curate"])]),
        vec!["media.serve"],
        "and a service filling something else does not stand in for it"
    );
}

/// The plugin itself does not count as something that would still be filling it,
/// which is the whole difference between asking about the machine now and asking
/// about the machine afterwards.
#[test]
fn the_plugin_going_is_not_read_as_something_that_stays() {
    let going = filling("komga", &["media.serve"]);

    assert_eq!(
        named(&going, std::slice::from_ref(&going), &[]),
        vec!["media.serve"],
        "it is in the register it is being removed from, and it is still going"
    );
}

/// A plugin that fills nothing takes nothing away with it, which is the common
/// answer and the one worth being sure of.
#[test]
fn a_plugin_that_fills_nothing_leaves_nothing_unfilled() {
    let going = filling("komga", &[]);
    assert!(named(&going, std::slice::from_ref(&going), &[]).is_empty());
}
