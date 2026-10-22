use lemonfiber_core::app::plugins::MoveOffered;

use super::offered;

/// Nothing on offer says nothing.
#[test]
fn nothing_on_offer_says_nothing() {
    assert!(offered(&[]).text().is_empty());
}

/// A move on offer names the part and the update that takes it.
#[test]
fn a_move_on_offer_names_the_part_and_the_reading_that_takes_it() {
    let said = offered(&[MoveOffered {
        plugin: "jellyfin".to_owned(),
        service: "jellyfin".to_owned(),
    }])
    .text();
    assert!(said.contains("runs jellyfin as a plugin"), "{said}");
    assert!(
        said.contains("`lemonfiber update stack` shows the move"),
        "{said}"
    );
}

/// A move taken says the plugin now runs in the service's place, and one not taken says
/// why it still runs as the stack's own.
#[test]
fn a_move_taken_or_not_says_which_and_why() {
    let said = super::moved(&[
        lemonfiber_core::update::run::Moved {
            plugin: "jellyfin".to_owned(),
            service: "jellyfin".to_owned(),
            taken: true,
            why: None,
        },
        lemonfiber_core::update::run::Moved {
            plugin: "bindery".to_owned(),
            service: "bindery".to_owned(),
            taken: false,
            why: Some("a value it sends needs approving".to_owned()),
        },
    ])
    .text();
    assert!(
        said.contains(
            "Moved: jellyfin now runs as its plugin in the place of the stack's own jellyfin."
        ),
        "{said}"
    );
    assert!(
        said.contains("Not moved: bindery still runs as the stack's own service, because a value it sends needs approving."),
        "{said}"
    );
}
