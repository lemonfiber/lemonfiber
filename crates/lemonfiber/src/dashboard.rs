//! The screen: where each panel goes, and what the whole frame looks like.
//!
//! Pure over a [`Snapshot`] and the space available. The terminal, the loop and
//! the keyboard are [`crate::terminal`]'s; nothing here knows a terminal exists,
//! which is what lets the whole screen be drawn and read back in a test.
//!
//! Laid out by what is available rather than by a fixed grid: a narrow terminal
//! drops to one column rather than squeezing two into a width neither fits, so it
//! degrades by carrying less at a time and never by overlapping.
//!
//! The panels reflow and so do the lines inside them: each panel is built for the
//! room its own place has, so nothing on the screen is decided by a width the
//! screen does not have.
//!
//! What an action has open is drawn over the panels rather than beside them, and
//! what it says is [`crate::acting`]'s. A running action opens nothing: the panels
//! are its report, and covering them would take away the one thing worth watching.

mod panels;

use lemonfiber_core::dashboard::{Snapshot, Telemetry};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::acting::{Acting, Pane};

/// The width below which two columns stop fitting.
///
/// Two panels of forty characters and their borders. Under it the screen carries
/// one column, which is less at a time and still correct — the alternative is a
/// layout that overlaps, and a corrupted screen is worse than a tall one.
const TWO_COLUMNS: u16 = 96;

/// The room a panel is built for where no screen is being drawn.
///
/// [`showing`] is asked what a snapshot says rather than what one terminal is
/// showing it at, and a word left out because a panel was narrow is not a word
/// this product declined to say.
const UNBOUNDED: usize = usize::MAX;

/// Draw the whole screen.
pub(crate) fn draw(frame: &mut Frame, snapshot: &Snapshot, acting: &Acting) {
    let area = frame.area();
    let [top, body, bottom] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .areas(area);

    // The screen's own state is read from the panels rather than carried in the
    // snapshot, so a panel that went down and the word for it cannot disagree.
    let telemetry = if panels::any_panel_down(snapshot) {
        Telemetry::Degraded
    } else {
        snapshot.telemetry
    };
    frame.render_widget(
        Paragraph::new(panels::header(
            telemetry,
            &snapshot.health,
            usize::from(top.width),
        )),
        top,
    );
    frame.render_widget(
        Paragraph::new(acting.footer(usize::from(bottom.width))),
        bottom,
    );

    let places = places(body);
    let panels = sections(snapshot, &rooms(&places));
    // Gathered before the panels are consumed by drawing, and only when it was
    // asked for.
    let showing = acting.showing_words().then(|| words_of(&panels));

    for (area, (title, lines)) in places.into_iter().zip(panels) {
        frame.render_widget(
            Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(title)),
            area,
        );
    }

    // Over the panels: what the action has open, where it has anything open.
    let (rows, across) = crate::pane::room_on(area);
    if let Some(open) = acting.pane(rows, across) {
        acted(frame, &open);
    }

    // Last, so it is over everything: the words on this screen, when asked for.
    // Read back from the panels' own lines rather than from the snapshot, so what
    // is explained is what is being shown.
    if let Some(showing) = showing {
        crate::pane::over(frame, &showing);
    }
}

/// Draw what an action has open over whatever is already drawn.
///
/// The same box the words are explained in, in the same place: two panes an
/// operator meets a keypress apart should not be two different shapes.
fn acted(frame: &mut Frame, open: &Pane) {
    let area = crate::pane::middle(frame.area());
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(open.lines.clone()).block(
            Block::default()
                .borders(Borders::ALL)
                .title(open.title.clone()),
        ),
        area,
    );
}

/// The words this screen is showing.
///
/// The titles as well as the lines: a panel called VPN has put that word on the
/// screen as surely as a line inside it would have.
fn words_of(panels: &[(&'static str, Vec<Line<'static>>)]) -> String {
    let titles: Vec<&str> = panels.iter().map(|(title, _)| *title).collect();
    let said = crate::pane::showing(panels.iter().flat_map(|(_, lines)| lines));
    format!("{} {said}", titles.join(" "))
}

/// The words a snapshot would put on the screen, for whoever needs them outside a
/// frame — the loop recording what an operator opened, which the drawing cannot do
/// because it happens every frame and this must happen once.
pub(crate) fn showing(snapshot: &Snapshot) -> String {
    words_of(&sections(snapshot, &[]))
}

/// The room each panel's own lines have, inside its border.
fn rooms(places: &[Rect]) -> Vec<usize> {
    places
        .iter()
        .map(|place| usize::from(place.width.saturating_sub(2)))
        .collect()
}

/// Each panel, in the order they are read: what is wrong first, then what is
/// happening, then what it is running on.
fn sections(snapshot: &Snapshot, rooms: &[usize]) -> Vec<(&'static str, Vec<Line<'static>>)> {
    // Taken in the order the panels are built below, which is the order their
    // places were laid out in — so no panel is ever built for another's width.
    let mut given = rooms.iter().copied().chain(std::iter::repeat(UNBOUNDED));
    let mut room = || given.next().unwrap_or(UNBOUNDED);
    let vpn = panels::vpn(snapshot.vpn.as_ref(), room());
    let transfers = panels::transfers(&snapshot.transfers, room());
    let queues = panels::queues(&snapshot.queue, room());
    let storage = panels::storage(&snapshot.storage, room());
    let services = panels::services(&snapshot.services, room());
    let stuck = panels::stuck(&snapshot.stuck, room());
    let alerts = panels::alerts(&snapshot.alerts, room());
    let door = panels::front_door(&snapshot.door, room());
    let household = panels::household(&snapshot.household, room());
    vec![
        ("VPN", vpn),
        ("Transfers", transfers),
        ("Queues", queues),
        ("Storage", storage),
        ("Services", services),
        ("Stuck", stuck),
        ("Alerts", alerts),
        ("Front door", door),
        ("Waiting on you", household),
    ]
}

/// Where the nine panels go in the space there is.
///
/// Two columns where the terminal is wide enough for both, one where it is not.
/// Every panel gets a place in either case: dropping one would leave an operator
/// looking for something that is simply not on the screen.
fn places(body: Rect) -> Vec<Rect> {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Ratio(1, 5); 5])
        .split(body);
    if body.width < TWO_COLUMNS {
        // One column: the same nine panels, stacked, each narrower and taller.
        return Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Ratio(1, 9); 9])
            .split(body)
            .to_vec();
    }
    let mut places = Vec::new();
    for (row, count) in rows.iter().zip([2usize, 2, 2, 2, 1]) {
        places.extend(
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints(vec![
                    Constraint::Ratio(1, u32::try_from(count).unwrap_or(1));
                    count
                ])
                .split(*row)
                .to_vec(),
        );
    }
    places
}

#[cfg(test)]
pub(crate) mod tests;
