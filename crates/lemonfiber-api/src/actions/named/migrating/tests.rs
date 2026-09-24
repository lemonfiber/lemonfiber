use super::{about_a_setup_already_here, named, EVERY};
use crate::actions::named::OFFERED;

/// The names a browser may ask for and the modes the survey offers are one list.
/// Two lists would drift the first time a fifth mode was added — and the drift would
/// be a mode the survey offers that no browser can reach, which reads as a mode that
/// does not work rather than one that was never wired up.
#[test]
fn every_mode_the_survey_offers_is_an_action_a_browser_can_ask_for() {
    for mode in EVERY {
        let name = format!("migrate-{}", mode.slug());
        assert!(OFFERED.contains(&name.as_str()), "{name} is offered");
    }
}

/// And nothing offered under that prefix is a mode nobody has heard of.
#[test]
fn every_migration_action_offered_names_a_mode() {
    let under_the_prefix: Vec<&&str> = OFFERED
        .iter()
        .filter(|name| name.starts_with("migrate-"))
        .collect();
    // A filter is where a sweep quietly stops sweeping: rename the prefix, or
    // take the migration actions out, and the loop below runs over nothing while
    // reporting that every one of them names a mode.
    assert!(
        !under_the_prefix.is_empty(),
        "nothing is offered under the prefix this is about"
    );
    for offered in under_the_prefix {
        assert!(named(offered).is_some(), "{offered} names a mode");
    }
}

/// A name that is not one of them is not one of them.
#[test]
fn an_action_about_something_else_names_no_mode() {
    assert!(named("uninstall").is_none());
    assert!(!about_a_setup_already_here("uninstall"));
}
