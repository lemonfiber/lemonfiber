//! The log viewer as it appears.
//!
//! Nothing here decides anything. Every question — what is shown, what is in force,
//! what the screen has had to give up — is answered by [`super::Viewer`], and what
//! is here only puts those answers where they can be read. The one judgement it
//! does make is colour, and it makes it from the severity a line declared rather
//! than from the stream it arrived on: this stack writes ordinary progress to
//! standard error, and a screen that paints all of that red teaches an operator to
//! ignore red.
//!
//! How much room each answer gets is settled here too. A row past the last column of
//! a terminal is not re-wrapped, it is not drawn, so every row this writes is laid
//! out for the width it was given: the lines wrap, the standing row and the keys take
//! as many rows as they need, and the title is built to fit the one it has.

use lemonfiber_core::logs::Level;
use lemonfiber_core::text::{wrapped, Overrun};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use super::{Shown, Viewer};

/// What the keys do, always on the screen.
///
/// An operator who has scrolled into history and cannot remember how to get back to
/// the tail is stuck on a screen that is still updating without them.
const KEYS: [&str; 8] = [
    "[/] filter",
    "[w] severity",
    "[s] service",
    "[c] clear",
    "[e] export",
    "[f] follow",
    "[?] words",
    "[q] quit",
];

/// What separates one key from the next.
const GAP: &str = "  ";

/// The least room a line is worth reading in, which the service column is measured
/// against: the column takes the room left over, never the room the line needs.
///
/// Forty is the dashboard's own figure for a column of text.
const LEAST: usize = 40;

/// What a line sits behind where the name that wrote it is on the row above.
const UNDER: &str = "  ";

/// Draw the whole screen.
pub(crate) fn draw(frame: &mut Frame, viewer: &Viewer) {
    let across = usize::from(frame.area().width);
    // Laid out before the body, whose height is what these two leave. Neither is
    // ever cut: the standing row carries the account of what the screen gave up, and
    // the keys carry the way out of a screen that is still updating.
    let standing = standing(viewer, across);
    let legend = legend(across);
    let [body, bottom, keys] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(tall(&standing)),
            Constraint::Length(tall(&legend)),
        ])
        .areas(frame.area());

    // Two rows of the body are its own border, and two of its columns are as well.
    // Asking for more lines than fit would silently drop the newest — which on a
    // tail is the ones being watched.
    let rows = usize::from(body.height.saturating_sub(2));
    let room = usize::from(body.width.saturating_sub(2));
    let shown = lines(viewer, rows, room);
    // Gathered before the lines are drawn, so what is explained is what is on the
    // screen rather than what the scrollback holds.
    let showing = viewer
        .glossary()
        .then(|| crate::pane::showing(shown.iter()));
    frame.render_widget(
        Paragraph::new(shown).block(
            Block::default()
                .borders(Borders::ALL)
                .title(Line::from(viewer.heading(room))),
        ),
        body,
    );
    frame.render_widget(Paragraph::new(standing), bottom);
    frame.render_widget(Paragraph::new(legend), keys);

    // Last, so it is over everything.
    if let Some(showing) = showing {
        crate::pane::over(frame, &showing);
    }
}

/// How many rows a stack of them asks the layout for.
fn tall(rows: &[Line<'static>]) -> u16 {
    u16::try_from(rows.len()).unwrap_or(u16::MAX)
}

/// What a row that is not a line's own words is shown in.
const fn dim() -> Style {
    Style::new().add_modifier(Modifier::DIM)
}

/// The body: the lines, or the reason there are none.
///
/// Handed back before it is drawn so the words on the screen can be found in it —
/// what is explained is what is being shown, not what the scrollback holds.
fn lines(viewer: &Viewer, rows: usize, room: usize) -> Vec<Line<'static>> {
    if let Some(reason) = viewer.nothing() {
        return wrapped(&reason, room, Overrun::Broken)
            .into_iter()
            .map(|row| Line::styled(row, dim()))
            .collect();
    }

    let named = widest(viewer.seen());
    // One line takes at least one row, so asking for a row's worth of lines asks for
    // at least as many as there is room to draw.
    let entries = viewer
        .showing(rows)
        .iter()
        .map(|shown| said(shown, viewer.colours(), named, room))
        .collect();
    fitted(entries, rows)
}

/// One line: who said it, and what they said, over as many rows as it takes.
///
/// The name is never shortened, wherever it goes: a shortened one no longer says
/// which service wrote the line. What the width decides is where it goes — beside the
/// line while the column and a readable line both fit, on a row of its own where they
/// do not.
fn said(shown: &Shown, colours: bool, named: usize, room: usize) -> Vec<Line<'static>> {
    let words = colour(shown.level, colours);
    if named + 1 + LEAST <= room {
        return beside(shown, named, room, words);
    }
    under(shown, room, words)
}

/// The name in its column, the line wrapped beside it and held to that column.
///
/// A wrapped row is indented to where the line starts rather than to the edge, so a
/// row continuing a line cannot be read as one beginning another.
fn beside(shown: &Shown, named: usize, room: usize, words: Style) -> Vec<Line<'static>> {
    let column = named + 1;
    parts(&shown.said, room.saturating_sub(column))
        .into_iter()
        .enumerate()
        .map(|(at, part)| {
            let head = if at == 0 {
                format!("{:<named$} ", shown.service)
            } else {
                " ".repeat(column)
            };
            Line::from(vec![Span::styled(head, dim()), Span::styled(part, words)])
        })
        .collect()
}

/// The name on a row of its own, the line wrapped underneath it.
fn under(shown: &Shown, room: usize, words: Style) -> Vec<Line<'static>> {
    let mut rows = vec![Line::styled(shown.service.clone(), dim())];
    rows.extend(
        parts(&shown.said, room.saturating_sub(UNDER.len()))
            .into_iter()
            .map(|part| Line::from(vec![Span::raw(UNDER), Span::styled(part, words)])),
    );
    rows
}

/// What one line breaks into, never fewer than one row.
///
/// An entry that came to no rows would take its name off the screen along with it,
/// which is what an empty line would otherwise do.
fn parts(said: &str, room: usize) -> Vec<String> {
    let parts = wrapped(said, room, Overrun::Broken);
    if parts.is_empty() {
        return vec![String::new()];
    }
    parts
}

/// As many whole lines as the body has rows for, oldest first.
///
/// Filled from the newest backwards: a paragraph given more rows than it has drops
/// the ones at the bottom, which on a tail is the newest.
///
/// A line that will not fit whole is left out rather than part-drawn, so a line stays
/// one thing — one step of the keys that page through them, and one entry on the
/// screen. A wrapped row drawn without the name above it belongs to whichever name
/// is above it, which is another service's. The exception is a line taller than the
/// whole body, which is shown as far as it goes.
fn fitted(entries: Vec<Vec<Line<'static>>>, rows: usize) -> Vec<Line<'static>> {
    let mut kept: Vec<Vec<Line<'static>>> = Vec::new();
    let mut taken = 0;
    for entry in entries.into_iter().rev() {
        if taken + entry.len() > rows {
            if kept.is_empty() {
                kept.push(entry.into_iter().take(rows).collect());
            }
            break;
        }
        taken += entry.len();
        kept.push(entry);
    }
    kept.into_iter().rev().flatten().collect()
}

/// The row that says what is in force and what has been given up — or, while a
/// search is being typed, the search itself.
///
/// Wrapped rather than cut. Both of the things it says are lost from the right: what
/// the screen gave up is said at the end of the row, and a search is at the point of
/// typing.
fn standing(viewer: &Viewer, across: usize) -> Vec<Line<'static>> {
    match viewer.typing() {
        Some(typed) => wrapped(&format!("/{typed}_"), across, Overrun::Broken)
            .into_iter()
            .map(Line::from)
            .collect(),
        None => wrapped(&viewer.footing(), across, Overrun::Broken)
            .into_iter()
            .map(|row| Line::styled(row, dim()))
            .collect(),
    }
}

/// The keys, over as many rows as the width needs.
///
/// Broken between keys and never inside one, so no row carries half a hint.
fn legend(across: usize) -> Vec<Line<'static>> {
    let mut rows: Vec<String> = Vec::new();
    for key in KEYS {
        match rows.last_mut() {
            Some(row) if row.chars().count() + GAP.len() + key.chars().count() <= across => {
                row.push_str(GAP);
                row.push_str(key);
            }
            _ => rows.push(key.to_owned()),
        }
    }
    rows.into_iter()
        .map(|row| Line::styled(row, dim()))
        .collect()
}

/// What colour a severity is shown in, where colour is allowed at all.
///
/// A line that declares none is left alone. Guessing from the stream it arrived on
/// would colour most of this stack's ordinary progress as failure, and a screen an
/// operator learns to disbelieve is worse than one with no colour at all.
///
/// Refusing colour costs nothing that matters, which is why it is safe to honour:
/// the severity is a **word in the line** — `WARN`, `ERROR` — so the screen says the
/// same thing either way and the colour was never carrying it alone. Dimming stays,
/// being an attribute rather than a colour, and the convention is about colour.
fn colour(level: Option<Level>, colours: bool) -> Style {
    match level {
        Some(Level::Error | Level::Fatal) if colours => Style::default().fg(Color::Red),
        Some(Level::Warn) if colours => Style::default().fg(Color::Yellow),
        Some(Level::Trace | Level::Debug) => dim(),
        _ => Style::default(),
    }
}

/// How wide the service column has to be to hold every name that will appear in it.
///
/// Measured rather than fixed, because a fixed column either wastes room on a stack
/// of short names or cuts the long ones — and a cut name no longer says which
/// service wrote the line, which is the one thing the column is there for.
/// `calibre-web-automated` and `audiobookshelf` both lose that at twelve
/// characters, and both are in the stack this ships with.
///
/// Grows only when a service first appears, so the column is steady while a stack
/// runs rather than shifting under whoever is reading it. It is never measured
/// against the terminal: a name too long for the room beside it moves rather than
/// shrinks, which [`said`] settles.
fn widest(seen: &[String]) -> usize {
    seen.iter()
        .map(|name| name.chars().count())
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
