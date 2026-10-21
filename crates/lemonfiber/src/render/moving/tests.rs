use lemonfiber_core::app::plugins::MoveOffered;

use super::offered;

/// Nothing on offer says nothing.
#[test]
fn nothing_on_offer_says_nothing() {
    assert!(offered(&[]).text().is_empty());
}

/// A move on offer names the part and the reading that takes it.
#[test]
fn a_move_on_offer_names_the_part_and_the_reading_that_takes_it() {
    let said = offered(&[MoveOffered {
        plugin: "jellyfin".to_owned(),
        service: "jellyfin".to_owned(),
    }])
    .text();
    assert!(said.contains("runs jellyfin as a plugin"), "{said}");
    assert!(
        said.contains("`lemonfiber plugin install jellyfin`"),
        "{said}"
    );
}
