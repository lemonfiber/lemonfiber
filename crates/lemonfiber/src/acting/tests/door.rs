//! The front door's code, opened full size over the panels.

use super::super::{door, Acting, Press};
use lemonfiber_core::dashboard::Panel;
use lemonfiber_core::door::{Address, Reaches};
use lemonfiber_core::model::{FrontDoorReport, Standing};
use ratatui::layout::Rect;

/// Where the household is sent in these tests.
const ADDRESS: &str = "http://kitchen-nas.local:5055";

/// A door with an address to draw a code from.
fn a_door() -> Panel<FrontDoorReport> {
    Panel::Ready(FrontDoorReport {
        standing: Standing::Established,
        chosen: None,
        service: None,
        address: Some(Address {
            url: ADDRESS.to_owned(),
            caution: None,
        }),
        reaches: Reaches::Here,
        unaddressed: None,
        facing: None,
        meaning: String::new(),
        beside: Vec::new(),
    })
}

/// Each line a pane says, as text.
fn said(acting: &Acting, screen: Rect) -> Vec<String> {
    let (rows, across) = crate::pane::room_on(screen);
    acting.pane(rows, across).map_or_else(Vec::new, |pane| {
        pane.lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect()
            })
            .collect()
    })
}

/// The box a key opens shows the front door's code whole, every row of it as it was
/// drawn, with the address under it.
#[test]
fn the_reading_box_shows_the_door_code_whole() {
    let mut acting = Acting::opened();
    let mut snapshot = crate::dashboard::tests::a_snapshot();
    snapshot.door = a_door();
    acting.landed(&snapshot);
    acting.pressed(&Press::Typed(door::KEY));

    let lines = said(&acting, Rect::new(0, 0, 120, 50));
    let drawn = crate::render::qr::rows(ADDRESS, crate::say::folding()).unwrap_or_default();
    assert!(!drawn.is_empty(), "an address this short always draws");
    let rows: Vec<String> = drawn.iter().map(|row| format!("  {row}")).collect();
    assert!(
        lines
            .windows(rows.len())
            .any(|window| window == rows.as_slice()),
        "the code was cut, or its rows moved apart: {lines:?}"
    );
    assert!(
        lines.contains(&format!("  {ADDRESS}")),
        "the address the code was drawn from is not under it: {lines:?}"
    );
}

/// With no address to draw from, the key opens nothing.
#[test]
fn with_no_address_the_key_opens_nothing() {
    let mut acting = Acting::opened();
    acting.at_the_door(&Panel::Pending);
    acting.pressed(&Press::Typed(door::KEY));

    assert!(said(&acting, Rect::new(0, 0, 120, 50)).is_empty());
}

/// Any key that is not a move puts the code away.
#[test]
fn any_other_key_puts_the_code_away() {
    let mut acting = Acting::opened();
    acting.at_the_door(&a_door());
    acting.pressed(&Press::Typed(door::KEY));
    acting.pressed(&Press::Forward);
    assert!(!said(&acting, Rect::new(0, 0, 120, 50)).is_empty());

    acting.pressed(&Press::Typed('x'));
    assert!(said(&acting, Rect::new(0, 0, 120, 50)).is_empty());
}
