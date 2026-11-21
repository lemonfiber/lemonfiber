use super::{consequential, decision, Cost, DecisionKey, DECISIONS};
use crate::config::{DATA_ROOT_KEY, FRONT_DOOR_KEY, VPN_PORT_FORWARDING_KEY};

/// Every decision names a setting once. Two rows for one key would let the surface
/// that reads this state one cost and apply the other.
#[test]
fn no_setting_is_catalogued_twice() {
    let mut keys: Vec<&str> = DECISIONS
        .iter()
        .map(|decision| decision.key.as_str())
        .collect();
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
            entry.key.as_str()
        );
        assert!(!entry.key.as_str().is_empty());
    }
}

/// The sharpest one in the product: moving the data without re-pointing the curators
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

/// Where each key stands in the catalogue, by a match a new key cannot compile past.
const fn position(key: DecisionKey) -> usize {
    match key {
        DecisionKey::DataRoot => 0,
        DecisionKey::MediaServerMode => 1,
        DecisionKey::Usenet => 2,
        DecisionKey::Torrent => 3,
        DecisionKey::Vpn => 4,
        DecisionKey::Puid => 5,
        DecisionKey::Pgid => 6,
        DecisionKey::VpnProvider => 7,
        DecisionKey::IndexerUrl => 8,
        DecisionKey::IndexerApikey => 9,
        DecisionKey::ProviderHost => 10,
        DecisionKey::ProviderPort => 11,
        DecisionKey::ProviderUser => 12,
        DecisionKey::ProviderPass => 13,
        DecisionKey::ProviderTls => 14,
        DecisionKey::FrontDoor => 15,
        DecisionKey::Zone => 16,
        DecisionKey::Locale => 17,
    }
}

/// The keys are the catalogue's, one to one and in its order, and the contract declares
/// exactly them, each by the name it is written under.
#[test]
fn every_key_is_one_row_of_the_catalogue_and_the_contract_declares_each_by_its_name() {
    for (at, entry) in DECISIONS.iter().enumerate() {
        assert_eq!(position(entry.key), at, "{}", entry.key.as_str());
        assert_eq!(
            serde_json::to_value(entry.key).ok(),
            Some(serde_json::Value::from(entry.key.as_str()))
        );
    }
    let declared: Vec<String> = serde_json::to_value(schemars::schema_for!(DecisionKey))
        .ok()
        .and_then(|schema| schema.get("oneOf").and_then(|one| one.as_array()).cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|key| {
            key.get("const")
                .and_then(|name| name.as_str())
                .map(str::to_owned)
        })
        .collect();
    let catalogued: Vec<String> = DECISIONS
        .iter()
        .map(|entry| entry.key.as_str().to_owned())
        .collect();
    assert_eq!(declared, catalogued);
}
