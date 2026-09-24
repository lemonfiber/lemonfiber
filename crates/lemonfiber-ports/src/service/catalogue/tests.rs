use super::CatalogueEntry;

/// An entry the way a catalogue returns one.
fn entry(reference: i64, held_as: Option<i64>) -> CatalogueEntry {
    CatalogueEntry {
        title: "Sintel".to_owned(),
        year: Some(2010),
        reference,
        held_as,
    }
}

#[test]
fn an_entry_the_service_already_holds_says_so() {
    // Detecting this is the whole of "already present must not be re-acquired": the
    // service's own id for it is the proof, not a title comparison.
    assert!(entry(1, Some(42)).is_already_here());
    assert!(!entry(1, None).is_already_here());
}

#[test]
fn an_entry_with_no_identifier_cannot_be_added() {
    // A catalogue result with no external id is a title and nothing else; asking the
    // service to take it on would be a request it cannot act on.
    assert!(entry(1234, None).is_addable());
    assert!(!entry(0, None).is_addable());
}

#[test]
fn an_entry_is_named_the_way_a_person_tells_two_apart() {
    assert_eq!(entry(1, None).named(), "Sintel (2010)");
    assert_eq!(
        CatalogueEntry {
            year: None,
            ..entry(1, None)
        }
        .named(),
        "Sintel"
    );
}
