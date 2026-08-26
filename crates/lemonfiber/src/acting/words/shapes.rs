//! The four shapes a box takes, whichever flow opened it.
//!
//! Five flows put boxes on this screen and between them they say a dozen different
//! things, but a box is only ever one of four arrangements: a list to move over, a
//! line to type on, an answer to read through, and a question under the account of
//! what answering it would come to. Those are here, apart from the words, because
//! they are the part no flow owns — an errand's list and a question's list are one
//! drawing over two lists, and the day they stopped being one is the day the screen
//! started behaving differently depending on which key opened it.
//!
//! Text from somewhere else — a form's own name, another service's account of what
//! went wrong — reaches the screen through [`shortened`] and never past it, so no
//! line can be put up by a route that skips being made safe for a terminal.

use lemonfiber_core::plural::s;
use lemonfiber_core::text::{fitted, plain};
use ratatui::text::{Line, Span};

use super::super::chooser::{Chooser, Listed};
use super::super::reading::Reading;
use crate::pane::quiet;

/// How a word a question has to be given is typed, and how to leave it.
const TYPING: &str = "enter asks   esc leaves it";

/// How a reading is moved through, and how it is put away.
const MOVING: &str = "up and down move   any other key closes";

/// How the cursor is moved over a list, whatever the list takes.
const OVER: &str = "up and down choose";

/// How a list is left, whatever it takes.
const AWAY: &str = "esc leaves it";

/// How a row is marked on a list that can take several.
const MARKS: &str = "space marks";

/// A row that has been marked to be taken with the others.
const MARKED: &str = "[x]";

/// A row on the same list that has not been.
///
/// An empty box rather than an empty column: what it says is that this row *can* be
/// marked and is not, which is the whole of what an operator needs to learn about a
/// key nobody told them existed. A blank there would read as a row that takes no mark
/// at all.
const UNMARKED: &str = "[ ]";

/// How the question before something that changes the stack is answered.
///
/// Only an explicit yes goes ahead, which is how the teardown's own question is read
/// too: an answer that is neither should land on the thing that changes nothing, and
/// on a screen where one key reaches an action it is a keypress that was not meant
/// that this is guarding against.
pub(super) const AGREEING: &str = "y goes ahead   any other key changes nothing";

/// The same, under an account the box also moves through.
const READING_AND_AGREEING: &str =
    "up and down move   y goes ahead   any other key changes nothing";

/// The entries, the selected one marked, and how to move over them.
pub(super) fn choosing<T: Listed>(
    chooser: &Chooser<T>,
    rows: usize,
    across: usize,
) -> Vec<Line<'static>> {
    // Two rows are kept back for the blank and the hint under the list, which is
    // what tells an operator that enter is what they are looking for.
    let room = rows.saturating_sub(2);
    let mut lines: Vec<Line<'static>> = chooser
        .listed()
        .take(room)
        .map(|(here, choice)| offered(here, choice, across))
        .collect();
    let left = chooser.listed().count().saturating_sub(lines.len());
    if left > 0 {
        lines.push(dimmed(
            &format!(
                "{left} more choice{} than this screen has room for",
                s(left)
            ),
            across,
        ));
    }
    lines.push(Line::raw(""));
    lines.push(dimmed(&hint(marks(chooser)), across));
    lines
}

/// How many rows are marked, or nothing where this list takes one.
///
/// Read off the entries rather than handed in, because whether a list takes several
/// is a fact about what is on it: a question and an errand cannot be taken together
/// and say so by having nowhere to put a mark.
fn marks<T: Listed>(chooser: &Chooser<T>) -> Option<usize> {
    let mut marked = None;
    for (_, entry) in chooser.listed() {
        if let Some(here) = entry.marked() {
            marked = Some(marked.unwrap_or(0) + usize::from(here));
        }
    }
    marked
}

/// What the line under a list says, which is what enter would do.
///
/// Stated rather than left to a rule somebody has to remember. A list that takes
/// several has two things enter can mean — the rows marked, or the row under the
/// cursor where none is — and the only moment that is ambiguous is the moment this
/// line resolves it.
fn hint(marked: Option<usize>) -> String {
    match marked {
        None => format!("{OVER}   enter goes on   {AWAY}"),
        Some(0) => format!("{OVER}   {MARKS}   enter takes this one   {AWAY}"),
        Some(several) => {
            format!("{OVER}   {MARKS}   enter takes the {several} marked   {AWAY}")
        }
    }
}

/// One entry on a list: where the cursor is, whether it is marked, and what it says.
fn offered(here: bool, entry: &impl Listed, across: usize) -> Line<'static> {
    let mark = match entry.marked() {
        None => String::new(),
        Some(true) => format!("{MARKED} "),
        Some(false) => format!("{UNMARKED} "),
    };
    row(
        &format!("{}{mark}", if here { "> " } else { "  " }),
        entry,
        across,
    )
}

/// One entry somewhere the cursor is not, which is where several are being named.
pub(super) fn named(entry: &impl Listed, across: usize) -> Line<'static> {
    row("  ", entry, across)
}

/// One row: whatever leads it, the name, and what it is for beside it.
fn row(leading: &str, entry: &impl Listed, across: usize) -> Line<'static> {
    // What leads the row and the two spaces after the name are taken off before the
    // name is fitted, so the row it ends up on is the width it was given rather than
    // that width plus whatever the lead cost. The whole of it is then fitted again,
    // because on a screen narrower than the lead itself the name has already been cut
    // to nothing and what is left to give back is the lead.
    let name = shortened(
        entry.name(),
        across.saturating_sub(leading.chars().count().saturating_add(2)),
    );
    let named = shortened(&format!("{leading}{name}  "), across);
    let room = across.saturating_sub(named.chars().count());
    Line::from(vec![
        Span::raw(named),
        Span::styled(shortened(entry.about(), room), quiet()),
    ])
}

/// What has to be given, and what has been typed of it so far.
///
/// The word is drawn as typed and never made to fit from the left, so what is being
/// typed stays where it was put — a field that scrolled under the operator's own
/// fingers would be a field nobody could correct.
pub(super) fn typing(asks: &str, typed: &str, across: usize) -> Vec<Line<'static>> {
    vec![
        Line::raw(shortened(asks, across)),
        Line::raw(shortened(&format!("> {typed}"), across)),
        Line::raw(""),
        dimmed(TYPING, across),
    ]
}

/// An answer, in the words the command line gives for the same question.
///
/// What is not on the screen is counted rather than left to be inferred from a box
/// that has stopped moving, because either end of a reading looks the same as a
/// reading that was short.
pub(super) fn read(reading: &Reading, rows: usize, across: usize) -> Vec<Line<'static>> {
    // Two rows are kept back for the blank and the hint under the answer, which is
    // what tells an operator the box moves at all.
    let room = rows.saturating_sub(2);
    let (shown, above, below) = reading.window(room);
    let mut lines: Vec<Line<'static>> = shown
        .into_iter()
        .map(|line| Line::raw(shortened(line, across)))
        .collect();
    if let Some(place) = elsewhere(above, below) {
        lines.push(dimmed(&place, across));
    }
    lines.push(Line::raw(""));
    lines.push(dimmed(MOVING, across));
    lines
}

/// A question, under the account of what answering it would come to.
///
/// Under it rather than over it: an effect somebody reads after they have agreed is
/// not something they agreed to, so what a reset would revert, what a bundle would
/// hold and what an upgrade would cost are the lines above the question rather than
/// the answer to it.
///
/// One drawing over every flow that asks one, for the reason the list is one drawing
/// over four lists: two flows arranging the same three parts differently is how one
/// screen becomes two, and this is the arrangement where the difference would decide
/// what somebody agreed to.
pub(super) fn agreed(
    asks: &str,
    about: &str,
    account: Option<&Reading>,
    rows: usize,
    across: usize,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    if let Some(account) = account {
        // Three rows are kept back for the question, the blank and the hint under
        // it, so the account never grows over the thing being agreed to.
        let (shown, above, below) = account.window(rows.saturating_sub(4));
        lines.extend(
            shown
                .into_iter()
                .map(|line| Line::raw(shortened(line, across))),
        );
        if let Some(place) = elsewhere(above, below) {
            lines.push(dimmed(&place, across));
        }
    }
    lines.push(Line::raw(shortened(
        &format!("{}?", asks.trim_end()),
        across,
    )));
    lines.push(dimmed(about, across));
    lines.push(Line::raw(""));
    lines.push(dimmed(
        if account.is_some() {
            READING_AND_AGREEING
        } else {
            AGREEING
        },
        across,
    ));
    lines
}

/// What is off the top and off the bottom of the box, or nothing where it holds
/// the whole answer.
pub(super) fn elsewhere(above: usize, below: usize) -> Option<String> {
    let over = format!("{above} more line{} above", s(above));
    let under = format!("{below} more line{} below", s(below));
    match (above, below) {
        (0, 0) => None,
        (0, _) => Some(under),
        (_, 0) => Some(over),
        _ => Some(format!("{over}, {under}")),
    }
}

/// A line that is not the one being read, drawn as such.
pub(super) fn dimmed(said: &str, across: usize) -> Line<'static> {
    Line::styled(shortened(said, across), quiet())
}

/// Text made safe for a terminal and then made to fit the row it has.
///
/// One place, so no line can be put on the screen by a route that skips either
/// half of it — the same rule the panels behind this one are held to.
pub(super) fn shortened(value: &str, room: usize) -> String {
    fitted(&plain(value), room)
}

#[cfg(test)]
mod tests {
    use super::{elsewhere, read};
    use crate::acting::reading::Reading;
    use ratatui::text::Line;

    /// A reading over nine numbered lines.
    fn nine() -> Reading {
        Reading::of((0..9).map(|at| format!("line {at}")).collect())
    }

    /// One line as text, its spans joined the way the screen shows them.
    fn text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<Vec<&str>>()
            .concat()
    }

    /// An answer longer than the box is moved through rather than cut, and what is
    /// off each end is counted — either end looks the same as a short answer
    /// otherwise, and an operator would read the end of the box as the end of it.
    #[test]
    fn an_answer_longer_than_the_screen_counts_what_is_off_each_end() {
        let mut reading = nine();

        let opened: Vec<String> = read(&reading, 4, 80).iter().map(text).collect();
        reading.forward();
        let moved: Vec<String> = read(&reading, 4, 80).iter().map(text).collect();
        let whole: Vec<String> = read(&nine(), 40, 80).iter().map(text).collect();

        assert!(opened.contains(&"line 0".to_owned()));
        assert!(opened
            .iter()
            .any(|line| line.contains("7 more lines below")));
        assert!(moved.contains(&"line 1".to_owned()));
        assert!(moved
            .iter()
            .any(|line| line.contains("1 more line above, 6 more lines below")));
        assert!(!whole.iter().any(|line| line.contains("more line")));
    }

    /// The end of an answer says only what is behind it, and the beginning only what
    /// is ahead: a box that counted nothing at either end would read as an answer
    /// that had stopped rather than one that had ended.
    #[test]
    fn the_end_of_an_answer_says_only_what_is_behind_it() {
        assert_eq!(elsewhere(0, 0), None);
        assert_eq!(elsewhere(8, 0), Some("8 more lines above".to_owned()));
        assert_eq!(elsewhere(1, 0), Some("1 more line above".to_owned()));
    }
}
