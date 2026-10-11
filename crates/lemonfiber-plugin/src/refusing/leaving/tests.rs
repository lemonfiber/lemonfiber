use crate::refusing::tests::{names, said, with};

/// What this build says about the fixture's service with `added` after its
/// configuration directory.
fn outbound(added: &str) -> Vec<String> {
    said(&with(added))
}

#[test]
fn a_service_saying_where_it_reaches_and_what_it_asks_for_is_read() {
    let said = outbound("reaches = \"comicvine.gamespot.com\"\nasks_for = \"Issue details\"\n");
    assert!(!names(&said, &["service komga.reaches"]), "{said:?}");
    assert!(!names(&said, &["service komga.asks_for"]), "{said:?}");
}

#[test]
fn a_service_saying_where_it_reaches_and_not_what_it_asks_for_is_refused() {
    let said = outbound("reaches = \"comicvine.gamespot.com\"\n");
    assert!(
        names(&said, &["service komga.asks_for", "is missing"]),
        "{said:?}"
    );
}

#[test]
fn a_service_saying_what_it_asks_for_and_not_where_it_reaches_is_refused() {
    let said = outbound("asks_for = \"Issue details\"\n");
    assert!(
        names(&said, &["service komga.reaches", "is missing"]),
        "{said:?}"
    );
}

#[test]
fn a_blank_asks_for_is_refused() {
    let said = outbound("reaches = \"\"\nasks_for = \"  \"\n");
    assert!(
        names(&said, &["service komga.asks_for", "is blank"]),
        "{said:?}"
    );
}

#[test]
fn an_unshown_character_in_what_it_asks_for_is_refused() {
    let said =
        outbound("reaches = \"comicvine.gamespot.com\"\nasks_for = \"Issue\u{202e}details\"\n");
    assert!(
        names(&said, &["service komga.asks_for", "U+202E"]),
        "{said:?}"
    );
}
