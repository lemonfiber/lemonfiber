//! The words on a full-screen view, explained on request.
//!
//! A report can put its words at the bottom, because a report ends. A full screen
//! does not: every row is already carrying something, and a footnote would have to
//! take a row away from what the operator came to look at — on the log viewer,
//! exactly the rows a flood is competing for.
//!
//! So the words are a keypress away instead. `?` opens this over whatever is there
//! and any key closes it again, which costs the screen nothing until it is asked
//! for. That is also the difference between an explanation offered and one imposed,
//! and it is the only shape of this that is dismissible without a setting.
//!
//! What it explains is **what is on the screen now** rather than everything this
//! product knows. A glossary of two dozen words is a document; the four words in
//! front of somebody is an answer.
//!
//! An explanation is prose, and the pane counts its own rows — so an explanation
//! wider than the pane is wrapped onto another row rather than shortened. An
//! explanation cut mid-sentence has stopped being one, and a pane whose whole
//! purpose is to say what a word means cannot afford to say most of it.

use lemonfiber_core::acknowledged::Acknowledged;
use lemonfiber_core::glossary::{mentioned, Term};
use lemonfiber_core::text::{wrapped, Overrun};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::{Frame, Line, Span, Style};
use ratatui::style::Modifier;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

/// How wide the pane is, as a share of the screen.
const WIDTH: u16 = 70;

/// What the pane calls itself where there is room to say it, longest first.
///
/// The width decides which of them is used, and every one of them is whole: a
/// title is the one row that cannot be given a second.
const TITLES: [&str; 2] = [
    " the words on this screen — any key closes ",
    " the words on this screen ",
];

/// What it is called where there is room for none of those. A pane has to be
/// called something, and half a name is not a name.
const SHORTEST: &str = " words ";

/// How tall, as a share, so it never covers everything behind it.
const HEIGHT: u16 = 60;

/// The dimmed style everything uncertain and everything secondary is drawn in.
///
/// An attribute rather than a colour, and shared by every full-screen view: a
/// terminal told to use no colour still dims, and two screens that each decided
/// this for themselves would eventually decide it differently.
pub(crate) fn quiet() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

/// The text a screen is showing, gathered so the words in it can be found.
///
/// Read back from the lines that were built rather than from the values they came
/// from, so what is explained is what is being shown — including anything a panel
/// worded for itself.
pub(crate) fn showing<'a>(lines: impl IntoIterator<Item = &'a Line<'a>>) -> String {
    lines
        .into_iter()
        .flat_map(|line| line.spans.iter().map(|span| span.content.as_ref()))
        .collect::<Vec<&str>>()
        .join(" ")
}

/// Draw the words on this screen over whatever is already drawn.
pub(crate) fn over(frame: &mut Frame, showing: &str) {
    let area = middle(frame.area());
    let (rows, across) = inside(area);
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(explaining(showing, rows, across)).block(
            Block::default()
                .borders(Borders::ALL)
                .title(titled(area.width)),
        ),
        area,
    );
}

/// The longest of the pane's names that fits the width it has.
///
/// The shortest is used where not even that fits, since a pane has to be called
/// something and half a name is not a name.
fn titled(across: u16) -> &'static str {
    let room = usize::from(across);
    TITLES
        .into_iter()
        .find(|title| title.chars().count() <= room)
        .unwrap_or(SHORTEST)
}

/// Every word on the screen it has room to explain, and a count of the rest.
///
/// Cut to what fits rather than scrolled: this is an aside, and an aside somebody
/// has to navigate has stopped being one. What goes is whole words rather than the
/// ends of their explanations, and what is left out is counted, because a pane that
/// quietly showed four of nine would be read as there being four.
fn explaining(showing: &str, rows: usize, across: usize) -> Vec<Line<'static>> {
    let used = mentioned(showing);
    if used.is_empty() {
        return broken("Nothing on this screen needs a word explaining.", across)
            .map(Line::raw)
            .collect();
    }

    let explained = explained_in(showing, rows, across, crate::render::glossary::known());
    let mut lines: Vec<Line<'static>> = explained
        .iter()
        .flat_map(|term| taught(term, across))
        .collect();

    let left = used.len().saturating_sub(explained.len());
    if left > 0 {
        lines.extend(
            counted(left, across, rows.saturating_sub(lines.len()))
                .into_iter()
                .map(|row| Line::styled(row, quiet())),
        );
    }
    lines
}

/// What the pane says about the words it had no room to explain, longest first.
///
/// The count is the half that has to survive: a pane that quietly showed two of six
/// would be read as there being two, so what goes as the room runs out is the
/// sentence around the number rather than the number.
fn more(left: usize) -> [String; 3] {
    [
        format!("and {left} more, which `lemonfiber explain` will say"),
        format!("and {left} more"),
        format!("+{left}"),
    ]
}

/// The longest of those that fits the rows there are for saying it.
///
/// Nothing at all where there is not a row for even the shortest: the pane has
/// already given every row it has to explaining words, which is what it is for.
fn counted(left: usize, across: usize, rows: usize) -> Vec<String> {
    more(left)
        .into_iter()
        .map(|said| broken(&said, across).collect::<Vec<String>>())
        .find(|said| said.len() <= rows)
        .unwrap_or_default()
}

/// Text over as many rows of the pane as it takes.
///
/// A screen is a grid of cells and past the edge is not re-wrapped, it is not
/// drawn — so a run with nothing to break on is broken at the edge here, where a
/// report would let it overrun and be re-wrapped by whatever is reading it.
fn broken(text: &str, across: usize) -> impl Iterator<Item = String> {
    wrapped(text, across.max(1), Overrun::Broken).into_iter()
}

/// One word and what it means, over as many rows as the explanation takes.
///
/// The word leads and its explanation is wrapped beside it, every row continuing
/// one indented to where it began so it cannot be read as another word's. The
/// column is capped at half the pane, so a long word never leaves its explanation
/// a strip too narrow to carry anything.
fn taught(term: &Term, across: usize) -> Vec<Line<'static>> {
    let column = (term.word.chars().count() + 2).min(across / 2);
    broken(term.short, across.saturating_sub(column))
        .enumerate()
        .map(|(at, part)| {
            let head = if at == 0 {
                Span::styled(
                    format!("{:<column$}", term.word),
                    Style::default().add_modifier(Modifier::BOLD),
                )
            } else {
                Span::raw(" ".repeat(column))
            };
            Line::from(vec![head, Span::raw(part)])
        })
        .collect()
}

/// The words this pane explains in full, given the room it has.
///
/// One place decides, because the loop that records what an operator opened must
/// record **exactly** what was explained to them. The ones the pane only names are
/// not explained — naming a word is how it stays findable, not how it gets taught —
/// and recording those would stop a later report explaining a word nobody ever read.
pub(crate) fn explained_in(
    showing: &str,
    rows: usize,
    across: usize,
    known: &Acknowledged,
) -> Vec<&'static Term> {
    let used = mentioned(showing);
    // A word already gone and found out about is named rather than taught again,
    // exactly as a report does it — so the room this pane has goes to what is new.
    // One rule, both surfaces: a screen and a report disagreeing about which words
    // somebody knows would be the same feature twice.
    let new: Vec<&'static Term> = used
        .iter()
        .copied()
        .filter(|term| !known.holds(term.word))
        .collect();
    if new.len() == used.len() && tall(&new, across) <= rows {
        return new;
    }
    // Measured against every word on the screen rather than against the ones left
    // out, which is never the larger number — so the row it is given is never
    // narrower than the row it takes.
    let budget = rows.saturating_sub(counted(used.len(), across, rows).len());
    let mut taken: Vec<&'static Term> = Vec::new();
    let mut height = 0;
    for term in new {
        let needs = taught(term, across).len();
        if height + needs > budget {
            break;
        }
        height += needs;
        taken.push(term);
    }
    taken
}

/// How many rows explaining all of these takes.
fn tall(terms: &[&'static Term], across: usize) -> usize {
    terms.iter().map(|term| taught(term, across).len()).sum()
}

/// How much room the pane has for words, on a screen of this size: rows and
/// columns both, since how many words fit depends on how wide each one runs.
pub(crate) fn room_on(screen: Rect) -> (usize, usize) {
    inside(middle(screen))
}

/// The words a pane opened over this screen would actually have taught.
///
/// **What it explained**, not what was on the screen. The pane names the words it
/// had no room for rather than dropping them, and a named word has not been taught —
/// so recording those would stop a later report explaining a word nobody ever read,
/// which is the one failure the whole record exists to avoid.
///
/// Here rather than at the loop that records it, because which words those are is a
/// decision and the loop is a terminal.
pub(crate) fn taught_on(showing: &str, screen: Rect) -> Vec<&'static str> {
    let (rows, across) = room_on(screen);
    explained_in(showing, rows, across, crate::render::glossary::known())
        .into_iter()
        .map(|term| term.word)
        .collect()
}

/// The rows and columns inside a pane of this size, its own border taken off.
fn inside(area: Rect) -> (usize, usize) {
    (
        usize::from(area.height.saturating_sub(2)),
        usize::from(area.width.saturating_sub(2)),
    )
}

/// A box in the middle of the screen, leaving what is behind it visible around.
pub(crate) fn middle(screen: Rect) -> Rect {
    let [_, row, _] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - HEIGHT) / 2),
            Constraint::Percentage(HEIGHT),
            Constraint::Percentage((100 - HEIGHT) / 2),
        ])
        .areas(screen);
    let [_, area, _] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - WIDTH) / 2),
            Constraint::Percentage(WIDTH),
            Constraint::Percentage((100 - WIDTH) / 2),
        ])
        .areas(row);
    area
}

#[cfg(test)]
mod tests;
