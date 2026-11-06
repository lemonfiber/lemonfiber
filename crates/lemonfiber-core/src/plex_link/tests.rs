use super::{consequence, crosses_owner, refusal, Mode, FRONTED_CHANGE, OWNER_COST};

#[test]
fn the_mode_is_managed_where_nothing_is_recorded_and_one_of_three_otherwise() {
    assert_eq!(Mode::held(None), Some(Mode::Managed));
    assert_eq!(Mode::held(Some("owner")), Some(Mode::Owner));
    assert_eq!(Mode::held(Some(" fronted ")), Some(Mode::Fronted));
    assert_eq!(Mode::held(Some("shared")), None);
    assert_eq!(refusal("managed"), None);
    assert!(refusal("shared")
        .is_some_and(|said| said.contains("managed, owner, fronted, and shared is none of them")));
}

#[test]
fn choosing_owner_or_fronted_says_what_it_costs_before_it_lands() {
    assert_eq!(consequence(Some("owner")).as_deref(), Some(OWNER_COST));
    assert_eq!(
        consequence(Some("fronted")).as_deref(),
        Some(FRONTED_CHANGE)
    );
    assert_eq!(consequence(Some("managed")), None);
    assert_eq!(consequence(None), None);
    assert_eq!(consequence(Some("shared")), None);
}

#[test]
fn only_entering_or_leaving_owner_changes_whose_account_the_owner_is() {
    assert!(crosses_owner(None, Some("owner")));
    assert!(crosses_owner(Some("owner"), Some("fronted")));
    assert!(crosses_owner(Some("owner"), None));
    assert!(!crosses_owner(Some("managed"), Some("fronted")));
    assert!(!crosses_owner(None, Some("fronted")));
    assert!(!crosses_owner(Some("owner"), Some("owner")));
}
