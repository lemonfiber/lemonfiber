//! The front-door panel: the address to hand somebody who lives here, and its code.

use super::{pending, shortened, unavailable};
use crate::pane::quiet;
use crate::render::door::standing;
use lemonfiber_core::dashboard::Panel;
use lemonfiber_core::model::FrontDoorReport;
use ratatui::text::{Line, Span};

/// The front door: the one address to hand somebody who lives here.
///
/// On the screen rather than behind a question, because the operator who needs it is
/// not the one who thought to ask — they have just been asked "what do I open?" by
/// somebody in the next room, and a screen that already has the answer should not
/// make them go and fetch it.
///
/// The address is its own line and the whole of it is shown, because an address is
/// only worth having if it can be read out; the name and the phrase beside it share
/// the line above, which is where the room is spent when there is not enough.
pub(in crate::dashboard) fn front_door(
    panel: &Panel<FrontDoorReport>,
    (room, rows): (usize, usize),
) -> Vec<Line<'static>> {
    let report = match panel {
        Panel::Ready(report) => report,
        Panel::Unavailable { reason } => return unavailable(reason, room),
        Panel::Pending => return pending(),
    };
    let phrase = standing(report.standing);
    let mut lines = vec![match &report.service {
        Some(service) => Line::from(vec![
            Span::raw(shortened(
                service,
                room.saturating_sub(phrase.chars().count() + 2),
            )),
            Span::raw("  "),
            Span::styled(phrase.to_owned(), quiet()),
        ]),
        None => Line::styled(phrase.to_owned(), quiet()),
    }];
    if let Some(address) = &report.address {
        lines.push(Line::raw(shortened(&address.url, room)));
    }
    // What the operator's own file did to this answer, where it did anything. A
    // refused setting that only appeared in the long answer would be one an operator
    // watching this screen never learned about.
    if let Some(said) = report
        .chosen
        .as_ref()
        .and_then(lemonfiber_core::door::Chosen::said)
    {
        lines.push(Line::styled(shortened(&said, room), quiet()));
    }
    if let Some(address) = report
        .address
        .as_ref()
        .filter(|_| rows != crate::dashboard::UNBOUNDED)
    {
        let left = rows.saturating_sub(lines.len());
        lines.extend(code(&address.url, room, left));
    }
    lines
}

/// The front door's code under its address where the panel has the room for it, and
/// where it has not, the key that opens it full size.
///
/// A code is drawn whole or not at all: a camera reads none of a code cut at the
/// panel's edge.
fn code(address: &str, room: usize, rows: usize) -> Vec<Line<'static>> {
    let Some(drawn) = crate::render::qr::rows(address, crate::say::folding()) else {
        return vec![Line::styled(
            shortened(crate::render::qr::TOO_LONG, room),
            quiet(),
        )];
    };
    let wide = drawn
        .iter()
        .map(|row| row.chars().count())
        .max()
        .unwrap_or(0);
    if wide <= room && drawn.len() <= rows {
        return drawn.into_iter().map(Line::raw).collect();
    }
    vec![Line::styled(
        shortened(
            &format!("No room for its code: press {}", crate::acting::door::KEY),
            room,
        ),
        quiet(),
    )]
}
