use lemonfiber_bundled::{Stage, PINNED};
use lemonfiber_manifest::Service;

use super::{offered, on_offer, MoveOffered, Offering};
use crate::config::paths::PLUGINS;
use crate::doctor::moving::Offers;
use crate::test_support::an_installed;

/// The services of the stack this build carries.
fn carried() -> Vec<Service> {
    crate::test_support::stack()
        .manifest()
        .map(|manifest| manifest.services)
        .unwrap_or_default()
}

/// The plugins the bundle installs.
fn installs() -> Vec<&'static str> {
    PINNED
        .iter()
        .filter(|one| one.stage == Some(Stage::Installed))
        .map(|one| one.first_party.plugin)
        .collect()
}

/// Every part the bundle installs that the carried stack runs as its own service is a move
/// on offer, under the service of its id.
#[test]
fn a_part_the_bundle_installs_that_the_stack_runs_itself_is_offered_a_move() {
    let stack = carried();
    let expected: Vec<MoveOffered> = installs()
        .into_iter()
        .filter(|plugin| stack.iter().any(|service| service.id == *plugin))
        .map(|plugin| MoveOffered {
            plugin: plugin.to_owned(),
            service: plugin.to_owned(),
        })
        .collect();

    assert!(
        !expected.is_empty(),
        "the carried stack runs no part the bundle installs"
    );
    assert_eq!(offered(PINNED, &stack, &[]), expected);
}

/// A part whose plugin is installed already has moved, so nothing is offered for it.
#[test]
fn a_part_whose_plugin_is_installed_is_offered_nothing() {
    let held: Vec<_> = installs()
        .into_iter()
        .map(|plugin| an_installed(plugin, Vec::new()))
        .collect();

    assert_eq!(offered(PINNED, &carried(), &held), Vec::new());
}

/// A stack that does not run a part's service has nothing to move.
#[test]
fn a_stack_without_the_parts_service_is_offered_nothing() {
    let without: Vec<Service> = carried()
        .into_iter()
        .filter(|service| !installs().contains(&service.id.as_str()))
        .collect();

    assert_eq!(offered(PINNED, &without, &[]), Vec::new());
}

/// A part the bundle assembles is the stack's own service and never a move.
#[test]
fn a_part_the_bundle_assembles_is_never_offered_a_move() {
    let assembled: Vec<&str> = PINNED
        .iter()
        .filter(|one| one.stage == Some(Stage::Assembled))
        .map(|one| one.first_party.plugin)
        .collect();

    assert!(offered(PINNED, &carried(), &[])
        .iter()
        .all(|one| !assembled.contains(&one.plugin.as_str())));
}

/// The offer is read from this machine's record each time it is asked: every move where
/// nothing is recorded installed, and nothing known where the record cannot be read.
#[tokio::test]
async fn the_offer_is_read_from_the_machines_record_each_time_it_is_asked() {
    let env = crate::test_support::env_without_password("moving-in-offering");
    let ctx = crate::test_support::a_context()
        .settings(crate::config::Settings {
            env_file: Some(env.clone()),
            ..crate::config::Settings::default()
        })
        .build();
    let Ok(manifest) = crate::test_support::stack().manifest() else {
        unreachable!("the carried stack has a manifest");
    };
    let offering = Offering::of(&ctx, &manifest);

    assert_eq!(
        offering.moves().await,
        Ok(offered(PINNED, &manifest.services, &[]))
    );

    let _ = std::fs::create_dir_all(env.with_file_name(PLUGINS));
    assert_eq!(offering.moves().await, Err(()));
    assert_eq!(on_offer(&ctx, &manifest), Vec::new());
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}
