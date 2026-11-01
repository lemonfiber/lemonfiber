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
        configured_by: None,
    }])
    .text();
    assert!(said.contains("runs jellyfin as a plugin"), "{said}");
    assert!(
        said.contains("`lemonfiber update stack` shows the move"),
        "{said}"
    );
}

/// A move whose kept part its adapter configures says what the part starts without.
#[test]
fn a_move_whose_adapter_configures_the_part_says_it_starts_without_that() {
    let said = offered(&[MoveOffered {
        plugin: "unpackerr".to_owned(),
        service: "unpackerr".to_owned(),
        configured_by: Some("unpackerr-adapter".to_owned()),
    }])
    .text();
    assert!(
        said.contains("Until it moves, unpackerr runs without unpackerr-adapter")
            && said.contains("the credentials it reaches other services with"),
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

/// A guard moved says it moved for good, one still to move says which run moves it, and
/// one refused says why and that it stays on offer.
#[test]
fn a_guard_moved_still_to_move_or_refused_says_which() {
    let guard = |taken, why: Option<&str>| lemonfiber_core::update::run::Moved {
        plugin: "decline".to_owned(),
        service: "decline".to_owned(),
        taken,
        why: why.map(str::to_owned),
    };
    let said = super::guards(&[
        guard(true, None),
        guard(false, None),
        guard(false, Some("a value it sends needs approving")),
    ])
    .text();
    assert!(
        said.contains("Moved for good: decline now runs as its plugin in the place of the stack's own decline. Neither an undo nor a removal puts it back."),
        "{said}"
    );
    assert!(
        said.contains("To move: decline moves onto its plugin decline with the next run that leaves the stack running, for good and with no yes of its own."),
        "{said}"
    );
    assert!(
        said.contains("Not moved: decline still runs as the stack's own service, because a value it sends needs approving. It stays on offer"),
        "{said}"
    );
    assert!(super::guards(&[]).text().is_empty());
}
