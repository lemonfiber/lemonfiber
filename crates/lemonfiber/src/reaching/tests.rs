use super::{reached, ACTS, ALSO, ASKS, OPENS, SHOWS};

/// Every request is reached one way, or a reader of the table has two rows'
/// worth of claim to reconcile against one row.
#[test]
fn no_request_is_reached_twice() {
    let every = reached();

    for request in &every {
        let same = every.iter().filter(|other| *other == request).count();
        assert_eq!(same, 1, "{request} is reached more than one way");
    }
    assert_eq!(
        every.len(),
        ACTS.len() + ASKS.len() + SHOWS.len() + OPENS.len()
    );
}

/// A read is named by its path and an action by a bare word, which is what tells
/// the two vocabularies apart wherever this list is read.
#[test]
fn a_question_is_named_by_a_path_and_an_action_by_a_word() {
    assert!(ASKS.iter().all(|reach| reach.through.starts_with("/api/")));
    assert!(ACTS.iter().all(|reach| !reach.through.contains('/')));
    assert!(ALSO.iter().all(|reach| !reach.through.contains('/')));
}

/// A second way is a second way to something already reached.
///
/// An entry here naming a request no other list holds would be a write nothing in
/// the parity table accounts for — the failure the list beside it exists to catch,
/// arriving through the list added to catch it.
#[test]
fn a_second_way_reaches_something_this_screen_already_reaches() {
    let every = reached();

    for reach in ALSO {
        assert!(
            every.contains(&reach.request),
            "{} is reached no other way",
            reach.request
        );
    }
    assert!(!ALSO.is_empty());
}
