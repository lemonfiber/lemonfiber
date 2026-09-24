use super::{fingerprint, Held, Inventory, Origin, Revealed, State, REVEALED, SHOULDER};

/// A credential entry with the state and advisory a test is about.
fn held(state: State, advisory: Option<&str>) -> Held {
    Held {
        name: "Usenet provider password".to_owned(),
        setting: "USENET_PASS".to_owned(),
        consumers: vec!["SABnzbd".to_owned()],
        location: "the settings file".to_owned(),
        origin: Origin::Operator,
        from: crate::origin::Origin::Bundled,
        state,
        fingerprint: None,
        advisory: advisory.map(ToOwned::to_owned),
    }
}

#[test]
fn every_state_is_named_and_no_two_share_a_name() {
    let states = [
        State::Absent,
        State::Active,
        State::Stale,
        State::Invalid,
        State::Rotating,
        State::Superseded,
    ];
    let mut names: Vec<&str> = states.iter().map(|state| state.as_str()).collect();
    let held = names.len();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), held, "{names:?}");
}

#[test]
fn the_states_worth_advising_are_the_ones_asking_for_a_decision() {
    assert!(State::Stale.worth_advising());
    assert!(State::Invalid.worth_advising());
    assert!(State::Absent.worth_advising());
    assert!(!State::Active.worth_advising());
    assert!(!State::Rotating.worth_advising());
    assert!(!State::Superseded.worth_advising());
}

#[test]
fn an_inventory_gathers_the_advisories_and_leaves_the_quiet_ones_out() {
    let inventory = Inventory::of(vec![
        held(State::Active, None),
        held(State::Stale, Some("never proven")),
    ]);

    assert_eq!(inventory.advisories(), vec!["never proven"]);
}

/// The state decides, not the sentence: a working credential carrying one says
/// nothing, which is what keeps the rule in one place.
#[test]
fn a_working_credential_carrying_an_advisory_still_says_nothing() {
    let inventory = Inventory::of(vec![held(State::Active, Some("left over"))]);

    assert!(inventory.advisories().is_empty());
}

#[test]
fn an_inventory_asked_for_nothing_else_carries_nothing_else() {
    let inventory = Inventory::of(vec![held(State::Active, None)]);

    assert!(inventory.rotated.is_none());
    assert!(inventory.revealed.is_none());
    assert!(!inventory.protection.against.is_empty());
}

#[test]
fn a_revealed_value_travels_beside_the_inventory_rather_than_inside_it() {
    let secret = format!("{}{}", "the-", "value-itself");
    let inventory = Inventory::of(vec![held(State::Active, None)]).showing(Revealed {
        name: "Usenet provider password".to_owned(),
        value: Some(secret.clone()),
        warning: REVEALED.to_owned(),
    });

    let listed = serde_json::to_string(&inventory.held).unwrap_or_default();
    assert!(
        !listed.contains(&secret),
        "the revealed value was listed inside the inventory"
    );
    assert!(inventory.revealed.is_some_and(|one| one.value.is_some()));
}

#[test]
fn both_warnings_say_what_printing_a_credential_costs() {
    for warning in [SHOULDER, REVEALED] {
        assert!(warning.contains("scrollback"), "{warning}");
    }
}

#[test]
fn a_fingerprint_is_short_marked_and_the_same_every_time() {
    let once = fingerprint("something");
    assert_eq!(once, fingerprint("something"));
    assert_eq!(once.len(), 5, "{once}");
    assert!(once.starts_with('~'), "{once}");
}

#[test]
fn two_different_values_are_told_apart_and_neither_is_recoverable() {
    let value = format!("{}{}", "the-", "key-value");
    let mark = fingerprint(&value);

    assert_ne!(mark, fingerprint("a-different-value"));
    assert!(
        !mark.contains(&value),
        "the value survived into its own fingerprint"
    );
    // The mark is the safe half of this pair and the only one worth quoting: five
    // characters chosen so the value cannot be read back out of them.
    assert!(
        !value.contains(mark.trim_start_matches('~')),
        "the fingerprint was lifted straight out of the value: {mark}"
    );
}

#[test]
fn the_fingerprint_of_nothing_is_still_a_fingerprint() {
    assert_eq!(fingerprint("").len(), 5);
}
