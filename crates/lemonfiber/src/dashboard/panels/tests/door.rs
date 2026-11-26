use super::{any_panel_down, front_door, said, UNDRAWN, WIDE};
use lemonfiber_core::dashboard::Panel;
use lemonfiber_core::door::{Address, Chosen, Facing, Refusal};
use lemonfiber_core::model::{FrontDoorReport, Standing};

/// The answer a stack with a working door gives.
fn door(chosen: Chosen, address: Option<Address>) -> FrontDoorReport {
    FrontDoorReport {
        standing: Standing::Established,
        chosen: Some(chosen),
        service: Some("Seerr".to_owned()),
        address,
        reaches: lemonfiber_core::door::Reaches::Here,
        unaddressed: None,
        facing: Some(Facing::Asking),
        meaning: "a sentence far too long for a panel".to_owned(),
        beside: Vec::new(),
    }
}

/// The door's code as the panel draws it, for an address short enough to draw.
fn drawn() -> Vec<String> {
    let drawn =
        crate::render::qr::rows(&reachable().url, crate::say::folding()).unwrap_or_default();
    assert!(!drawn.is_empty(), "an address this short always draws");
    drawn
}

/// The address a machine that says what it is called is reached at.
fn reachable() -> Address {
    Address {
        url: "http://kitchen-nas.local:5055".to_owned(),
        caution: None,
    }
}

#[test]
fn the_screen_carries_the_address_to_hand_the_household() {
    // The whole point of the panel: the operator asked "what do I open?" by
    // somebody in the next room reads it off the screen they already have open.
    let lines = said(&front_door(
        &Panel::Ready(door(Chosen::Derived, Some(reachable()))),
        UNDRAWN,
    ));
    assert_eq!(
        lines,
        vec![
            "Seerr  the front door".to_owned(),
            "http://kitchen-nas.local:5055".to_owned(),
        ]
    );
}

#[test]
fn a_door_with_no_address_shows_the_standing_and_no_line_to_read_out() {
    let lines = said(&front_door(
        &Panel::Ready(FrontDoorReport {
            standing: Standing::Stranded,
            address: None,
            ..door(Chosen::Derived, None)
        }),
        UNDRAWN,
    ));
    assert_eq!(
        lines,
        vec!["Seerr  the front door, and there is no address to arrive at".to_owned()]
    );
}

#[test]
fn a_stack_with_no_door_says_so_rather_than_showing_an_empty_panel() {
    let lines = said(&front_door(
        &Panel::Ready(FrontDoorReport {
            standing: Standing::Absent,
            service: None,
            facing: None,
            ..door(Chosen::Derived, None)
        }),
        UNDRAWN,
    ));
    assert_eq!(lines, vec!["There is no front door.".to_owned()]);
}

#[test]
fn a_setting_that_was_refused_is_on_the_screen_rather_than_only_in_the_answer() {
    // An operator watching this screen never asks the long question, so a
    // refusal that only appeared there is one they would never learn about.
    let lines = said(&front_door(
        &Panel::Ready(door(
            Chosen::Refused(Refusal {
                named: "sonarr".to_owned(),
                because: "it answers this machine alone".to_owned(),
            }),
            Some(reachable()),
        )),
        UNDRAWN,
    ));
    assert_eq!(
        lines.last().map(String::as_str),
        Some("sonarr was named as the front door and cannot be one")
    );
}

#[test]
fn a_door_the_operator_named_says_that_it_was_named() {
    let lines = said(&front_door(
        &Panel::Ready(door(
            Chosen::Named("jellyfin".to_owned()),
            Some(reachable()),
        )),
        UNDRAWN,
    ));
    assert_eq!(
        lines.last().map(String::as_str),
        Some("named rather than worked out")
    );
}

#[test]
fn a_door_panel_that_could_not_be_filled_says_why_and_marks_the_screen() {
    let mut snapshot = crate::dashboard::tests::a_snapshot();
    snapshot.door = Panel::unavailable("the stack could not be read");
    assert!(any_panel_down(&snapshot));
    let lines = said(&front_door(&snapshot.door, UNDRAWN));
    assert_eq!(
        lines,
        vec!["unavailable — the stack could not be read".to_owned()]
    );
}

/// A panel with the room for it draws the door's code whole under the address.
#[test]
fn a_panel_with_room_draws_the_code_whole() {
    let drawn = drawn();
    let lines = said(&front_door(
        &Panel::Ready(door(Chosen::Derived, Some(reachable()))),
        (WIDE, 2 + drawn.len()),
    ));

    assert_eq!(lines.get(2..), Some(drawn.as_slice()), "{lines:?}");
}

/// A panel too small for the code says so and names the key that opens it full size,
/// rather than drawing part of it.
#[test]
fn a_panel_too_small_for_the_code_names_the_key_that_opens_it() {
    let drawn = drawn();
    let wide = drawn.first().map_or(0, |row| row.chars().count());
    for room in [(WIDE, 1 + drawn.len()), (wide - 1, 60)] {
        let lines = said(&front_door(
            &Panel::Ready(door(Chosen::Derived, Some(reachable()))),
            room,
        ));

        assert!(
            lines.iter().all(|line| !drawn.contains(line)),
            "part of a code was drawn: {lines:?}"
        );
        assert!(
            lines
                .last()
                .is_some_and(|line| line.contains(&format!("press {}", crate::acting::door::KEY))),
            "{lines:?}"
        );
    }
}

/// An address too long for a code says so in the panel rather than drawing nothing.
#[test]
fn a_panel_with_an_address_too_long_for_a_code_says_so() {
    let far_too_long = Address {
        url: format!("http://{}.local", "h".repeat(8000)),
        caution: None,
    };
    let lines = said(&front_door(
        &Panel::Ready(door(Chosen::Derived, Some(far_too_long))),
        (WIDE, 60),
    ));

    assert!(
        lines
            .last()
            .is_some_and(|line| line == crate::render::qr::TOO_LONG),
        "{lines:?}"
    );
}
