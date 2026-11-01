use lemonfiber_bundled::{Stage, PINNED};
use lemonfiber_manifest::Service;

use super::{offered, on_offer, MoveOffered, Offering, Ran};
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

/// A machine where every part the bundle installs has run.
fn all_ran() -> Ran {
    Ran::of(None, installs().into_iter().map(str::to_owned).collect())
}

/// Every part the bundle installs that the carried stack runs as its own service is a move
/// on offer, under the service of its id.
#[test]
fn a_part_the_bundle_installs_that_the_stack_runs_itself_is_offered_a_move() {
    let stack = carried();
    let expected: Vec<MoveOffered> = installs()
        .into_iter()
        .filter(|plugin| stack.iter().any(|service| service.id == *plugin))
        .collect::<Vec<_>>()
        .into_iter()
        .map(|plugin| (plugin, offered(PINNED, &stack, &[], &all_ran())))
        .filter_map(|(plugin, all)| all.into_iter().find(|one| one.plugin == plugin))
        .collect();

    assert!(
        !expected.is_empty(),
        "the carried stack runs no part the bundle installs"
    );
    assert_eq!(offered(PINNED, &stack, &[], &all_ran()), expected);
    assert!(expected.iter().all(|one| one.service == one.plugin));
}

/// A kept part whose adapter writes the configuration it reads starts without what that
/// holds until it moves, its curator keys among it, and the offer names the adapter; a
/// part whose adapter writes nothing of it names none.
#[test]
fn a_kept_part_configured_by_its_adapter_is_offered_naming_that_adapter() {
    let stack = carried();
    let configured = |plugin: &str| {
        offered(PINNED, &stack, &[], &all_ran())
            .into_iter()
            .find(|one| one.plugin == plugin)
            .map(|one| one.configured_by)
    };
    assert_eq!(
        configured("unpackerr"),
        Some(Some("unpackerr-adapter".to_owned()))
    );
    assert_eq!(configured("jellyfin"), Some(None));
    let unpackerr = stack.iter().find(|service| service.id == "unpackerr");
    assert!(unpackerr.is_some_and(
        |service| service.secret.is_empty() && !crate::project::names_a_secret(&service.env)
    ));
}

/// A part whose plugin is installed already has moved, so nothing is offered for it.
#[test]
fn a_part_whose_plugin_is_installed_is_offered_nothing() {
    let held: Vec<_> = installs()
        .into_iter()
        .map(|plugin| an_installed(plugin, Vec::new()))
        .collect();

    assert_eq!(offered(PINNED, &carried(), &held, &all_ran()), Vec::new());
}

/// A stack that does not run a part's service has nothing to move.
#[test]
fn a_stack_without_the_parts_service_is_offered_nothing() {
    let without: Vec<Service> = carried()
        .into_iter()
        .filter(|service| !installs().contains(&service.id.as_str()))
        .collect();

    assert_eq!(offered(PINNED, &without, &[], &all_ran()), Vec::new());
}

/// A part the bundle assembles is the stack's own service and never a move.
#[test]
fn a_part_the_bundle_assembles_is_never_offered_a_move() {
    let assembled: Vec<&str> = PINNED
        .iter()
        .filter(|one| one.stage == Some(Stage::Assembled))
        .map(|one| one.first_party.plugin)
        .collect();

    assert!(offered(PINNED, &carried(), &[], &all_ran())
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
        Ok(Vec::new()),
        "nothing has run here"
    );

    let _ = std::fs::create_dir_all(env.with_file_name(PLUGINS));
    assert_eq!(offering.moves().await, Err(()));
    assert_eq!(on_offer(&ctx, &manifest).await, Vec::new());
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A part whose stack service has never run here is no move: neither its configuration
/// directory holds anything nor does the engine hold a container for it.
#[test]
fn a_part_never_run_here_is_offered_no_move() {
    let stack = lemonfiber_fixtures::scratch::Scratch::unmade("moving-in-ran");
    let config = crate::plugin::configuration(&stack, "jellyfin");
    assert!(std::fs::create_dir_all(&config).is_ok());
    let ran = Ran::of(Some(stack.to_path_buf()), vec!["sonarr".to_owned()]);
    let moved = |ran: &Ran| -> Vec<String> {
        offered(PINNED, &carried(), &[], ran)
            .into_iter()
            .map(|one| one.plugin)
            .collect()
    };

    assert_eq!(moved(&ran), ["sonarr"], "an empty directory is no run");
    assert!(std::fs::write(config.join("system.xml"), "<x/>").is_ok());
    let mut both = moved(&ran);
    both.sort();
    assert_eq!(both, ["jellyfin", "sonarr"]);
    assert!(moved(&Ran::default()).is_empty());
}
