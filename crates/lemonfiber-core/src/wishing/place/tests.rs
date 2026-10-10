use base64::Engine as _;

use super::{Place, PAGES_AT_MOST, SHOWN_AT_MOST};

/// The code a place is refused with, or nothing where it reads.
fn refused(after: &str) -> Option<String> {
    Place::read(Some(after))
        .err()
        .map(|problem| problem.code.to_string())
}

/// `json` as a place is written.
fn written(json: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json)
}

#[test]
fn a_search_begins_at_the_first_page_having_shown_nothing() {
    assert_eq!(
        Place::read(None).ok(),
        Some(Place {
            page: 1,
            shown: Vec::new(),
        })
    );
}

#[test]
fn a_place_written_by_one_page_is_read_back_by_the_next() {
    let place = Place {
        page: 3,
        shown: vec!["603".to_owned(), "1399".to_owned()],
    };
    let after = place.written();
    assert_eq!(
        after
            .as_deref()
            .and_then(|after| Place::read(Some(after)).ok()),
        Some(place)
    );
}

#[test]
fn a_place_no_page_could_have_given_is_refused() {
    for after in [
        "not base64 at all!".to_owned(),
        written("not json"),
        written(r#"{"page":1,"shown":[]}"#),
        written(r#"{"page":0,"shown":[]}"#),
        written(&format!(r#"{{"page":{},"shown":[]}}"#, PAGES_AT_MOST + 1)),
        written(r#"{"page":2,"shown":[],"more":true}"#),
        "a".repeat(16 * 1024 + 1),
    ] {
        assert_eq!(refused(&after).as_deref(), Some("WISH-2"), "{after:.40}");
    }
    let too_many = (0..=SHOWN_AT_MOST)
        .map(|id| format!("\"{id}\""))
        .collect::<Vec<_>>()
        .join(",");
    assert_eq!(
        refused(&written(&format!(r#"{{"page":2,"shown":[{too_many}]}}"#))).as_deref(),
        Some("WISH-2")
    );
}

#[test]
fn a_search_ends_past_its_last_page_or_with_more_shown_than_a_place_carries() {
    let past = Place {
        page: PAGES_AT_MOST + 1,
        shown: Vec::new(),
    };
    let crowded = Place {
        page: 2,
        shown: (0..=SHOWN_AT_MOST).map(|id| id.to_string()).collect(),
    };
    let long = Place {
        page: 2,
        shown: vec!["x".repeat(16 * 1024)],
    };
    let last = Place {
        page: PAGES_AT_MOST,
        shown: Vec::new(),
    };
    assert_eq!(past.written(), None);
    assert_eq!(crowded.written(), None);
    assert_eq!(long.written(), None);
    assert!(last.written().is_some());
}
