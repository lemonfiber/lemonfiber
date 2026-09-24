use super::{consequential, decision, Cost, DECISIONS};
use crate::config::{DATA_ROOT_KEY, FRONT_DOOR_KEY, VPN_PORT_FORWARDING_KEY};

/// Every decision names a setting once. Two rows for one key would let the surface
/// that reads this state one cost and apply the other.
#[test]
fn no_setting_is_catalogued_twice() {
    let mut keys: Vec<&str> = DECISIONS.iter().map(|decision| decision.key).collect();
    let total = keys.len();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), total);
}

/// The cost is only half of it: a consequence the operator cannot act on is not
/// worth stating, so every row says what changing it affects, in words.
#[test]
fn every_decision_says_what_changing_it_affects() {
    assert!(!DECISIONS.is_empty());
    for entry in &DECISIONS {
        assert!(
            entry.affects.len() > 20,
            "{} says too little to act on",
            entry.key
        );
        assert!(!entry.key.is_empty());
    }
}

/// The sharpest one in the product: moving the data without re-pointing the *arrs
/// leaves a library that points at nothing, and that has to be said beforehand.
#[test]
fn moving_the_data_location_is_consequential_and_says_why() {
    assert!(consequential(DATA_ROOT_KEY));
    let moved = decision(DATA_ROOT_KEY);
    assert!(moved.is_some_and(|entry| entry.affects.contains("points at nothing")));
}

/// Naming a front door changes what one link points at and nothing else. Treating
/// it like moving a library teaches the operator to dismiss the confirmations that
/// matter.
#[test]
fn naming_the_front_door_is_cheap() {
    assert!(!consequential(FRONT_DOOR_KEY));
    assert!(decision(FRONT_DOOR_KEY).is_some_and(|entry| entry.cost == Cost::Cheap));
}

/// A setting setup never asked about is not a decision this catalogue speaks for.
#[test]
fn a_setting_setup_never_asked_about_is_not_a_decision_here() {
    assert!(decision("LEMONFIBER_NOT_A_SETTING").is_none());
    assert!(!consequential("LEMONFIBER_NOT_A_SETTING"));
    // Read by the product, but never written by setup: the forwarded port already
    // states its own consequence where it has one.
    assert!(decision(VPN_PORT_FORWARDING_KEY).is_none());
}
