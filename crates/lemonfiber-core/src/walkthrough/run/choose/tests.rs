use crate::ports::service::CatalogueEntry;
use crate::recyclarr::Kind;
use crate::walkthrough::Suggestion;

#[test]
fn nothing_asked_for_falls_back_to_the_safest_thing_this_stack_can_handle() {
    // The fallback is what an operator with an empty library actually gets, so it has
    // to be something the running services could file.
    let television = Suggestion::safest(&[Kind::Sonarr]).map(|s| s.kind);
    assert_eq!(television, Some(Kind::Sonarr));
    assert_eq!(Suggestion::safest(&[]), None);
}

#[test]
fn the_already_here_test_is_the_services_own_id_and_not_a_title_comparison() {
    // Two films share a name more often than anyone expects; the service's own id for
    // something is the only proof that it is holding this one.
    let held = CatalogueEntry {
        title: "Sintel".to_owned(),
        year: Some(2010),
        reference: 45_745,
        held_as: Some(7),
    };
    assert!(held.is_already_here());
    assert!(!CatalogueEntry {
        held_as: None,
        ..held
    }
    .is_already_here());
}
