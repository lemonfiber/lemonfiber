//! Every action as the contract lists it.

use super::{every, CONSENT};
use crate::actions::asked::TAKEN;
use crate::actions::named::OFFERED;

/// Every action the surface offers is listed, once, in the order it offers them.
#[test]
fn every_action_offered_is_listed_once_in_order() {
    let listed: Vec<&str> = every(|_| false).iter().map(|one| one.action).collect();
    assert_eq!(listed, OFFERED);
}

/// An action lists exactly the arguments its command has somewhere to put, each with
/// the type the carrier reads it as.
#[test]
fn an_action_lists_the_arguments_it_takes_with_their_types() {
    let actions = every(|_| false);
    let restore = actions.iter().find(|one| one.action == "restore");
    let names: Vec<&str> = restore
        .map(|one| one.arguments.iter().map(|argument| argument.name).collect())
        .unwrap_or_default();
    let expected: Vec<&str> = TAKEN
        .iter()
        .filter(|taken| taken.takers.contains(&"restore"))
        .map(|taken| taken.name)
        .collect();
    assert_eq!(names, expected);
    assert!(names.contains(&"archive"), "{names:?}");
    for argument in actions.iter().flat_map(|one| &one.arguments) {
        assert!(
            !argument.shape.is_null(),
            "{} has no type in the carrier's schema",
            argument.name
        );
    }
}

/// A flag read from a bare word is published as one, whatever enum reads it.
#[test]
fn a_flag_read_from_a_bare_word_is_published_as_a_boolean() {
    let actions = every(|_| false);
    let wait = actions
        .iter()
        .flat_map(|one| &one.arguments)
        .find(|argument| argument.name == "wait")
        .map(|argument| argument.shape.clone());
    assert_eq!(
        wait.as_ref().and_then(|shape| shape.get("type")),
        Some(&serde_json::json!("boolean"))
    );
}

/// The consent an action asks for is the arguments it takes that carry a yes, and an
/// action that takes none of them asks nothing first.
#[test]
fn the_consent_an_action_asks_for_is_the_yes_its_arguments_carry() {
    let actions = every(|_| false);
    let consent = |action: &str| {
        actions
            .iter()
            .find(|one| one.action == action)
            .map(|one| one.consent.clone())
            .unwrap_or_default()
    };
    assert!(
        consent("repair").contains(&"offer"),
        "{:?}",
        consent("repair")
    );
    assert!(consent("repair").contains(&"agreed"));
    assert!(consent("plugin-install").contains(&"approved"));
    assert!(consent("up").is_empty());
    for one in &actions {
        assert!(one.consent.iter().all(|name| CONSENT.contains(name)));
    }
}

/// Whether an action takes a rehearsal is the core's answer, passed through.
#[test]
fn a_rehearsal_is_what_the_core_says_of_the_action() {
    let actions = every(|action| action == "update");
    assert!(actions
        .iter()
        .all(|one| one.rehearsal == (one.action == "update")));
}

/// Every argument says what leaving it out reads as, in its own type, because the
/// carrier fills in every field a request leaves out; a flag read from a bare word
/// defaults to false.
#[test]
fn every_argument_defaults_to_a_value_of_its_own_type() {
    for argument in every(|_| false).iter().flat_map(|one| &one.arguments) {
        let default = argument.shape.get("default");
        assert!(default.is_some(), "{} names no default", argument.name);
        if argument.shape.get("type") == Some(&serde_json::json!("boolean")) {
            assert_eq!(
                default,
                Some(&serde_json::json!(false)),
                "{} is a flag",
                argument.name
            );
        }
    }
}
