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
mod tests {
    use super::{
        explained_in, explaining, middle, room_on, showing, taught, taught_on, titled, SHORTEST,
        TITLES,
    };
    use lemonfiber_core::acknowledged::Acknowledged;
    use lemonfiber_core::glossary::{mentioned, TERMS};
    use ratatui::layout::Rect;
    use ratatui::prelude::{Line, Span};

    /// A pane with more room than anything here is testing the edge of.
    const WIDE: usize = 200;

    /// The text of one line, for an assertion to read it back.
    fn text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<Vec<&str>>()
            .concat()
    }

    /// What is explained is what is being shown, read back from the lines that were
    /// built rather than from the values behind them.
    #[test]
    fn the_words_explained_are_the_words_on_the_screen() {
        let drawn = [
            Line::from(vec![Span::raw("no indexer answered")]),
            Line::from(vec![Span::raw("nothing here needs saying")]),
        ];

        let said = showing(drawn.iter());
        let explained = explaining(&said, 10, WIDE);

        assert_eq!(explained.len(), 1, "one word was on the screen");
        let first = explained.first().map(text).unwrap_or_default();
        assert!(first.starts_with("indexer"), "{first}");
    }

    /// A screen with none of them says so, rather than opening an empty box that
    /// reads as something having gone wrong.
    #[test]
    fn a_screen_with_no_words_to_explain_says_so() {
        let drawn = [Line::from(vec![Span::raw("everything is running")])];

        let explained = explaining(&showing(drawn.iter()), 10, WIDE);

        assert_eq!(explained.len(), 1);
        let said = explained.first().map(text).unwrap_or_default();
        assert!(said.contains("Nothing on this screen"), "{said}");
    }

    /// A pane that quietly showed four of nine would be read as there being four.
    #[test]
    fn what_did_not_fit_is_counted_rather_than_dropped() {
        let drawn = [Line::from(vec![Span::raw(
            "the indexer, the hardlink, the VPN, the ratio and the seed",
        )])];

        let explained = explaining(&showing(drawn.iter()), 4, WIDE);

        let last = explained.last().map(text).unwrap_or_default();
        assert!(last.starts_with("and 2 more"), "{last}");
    }

    /// The loop that records what was opened must record exactly what was
    /// explained — a word the pane only named has not been taught.
    #[test]
    fn only_the_words_it_explained_count_as_explained() {
        let said = "the indexer, the hardlink, the VPN, the ratio and the seed";

        let nothing = Acknowledged::default();
        let roomy = explained_in(said, 10, WIDE, &nothing);
        let cramped = explained_in(said, 3, WIDE, &nothing);

        let (roomy_count, cramped_count) = (roomy.len(), cramped.len());
        assert!(
            roomy_count > cramped_count,
            "room decides how many: {roomy_count} against {cramped_count}"
        );
        assert_eq!(cramped.len(), 2, "one line goes to the ones it only names");
        assert!(cramped.iter().all(|term| roomy.contains(term)));
    }

    /// One rule, both surfaces. What a report declines to teach again a pane
    /// declines too — the two disagreeing about which words somebody knows would
    /// be the same feature written twice, differently.
    #[test]
    fn a_word_already_known_is_named_here_as_it_is_in_a_report() {
        let mut known = Acknowledged::default();
        known.take("indexer");

        let explained = explained_in("no indexer answered, the hardlink failed", 10, WIDE, &known);

        let words: Vec<&str> = explained.iter().map(|term| term.word).collect();
        assert_eq!(words, ["hardlink"], "the room goes to what is new");
    }

    /// What is recorded is what the pane taught, and it is measured against the
    /// pane's own room rather than the screen's — a word left out for want of a row
    /// is one nobody read.
    #[test]
    fn the_words_recorded_are_the_ones_the_pane_had_room_to_teach() {
        let said = "the indexer, the hardlink, the VPN, the ratio and the seed";

        let roomy = taught_on(said, Rect::new(0, 0, 200, 60));
        let cramped = taught_on(said, Rect::new(0, 0, 200, 8));

        let (roomy_count, cramped_count) = (roomy.len(), cramped.len());
        assert!(roomy.contains(&"indexer"), "{roomy:?}");
        assert!(
            roomy_count > cramped_count,
            "room decides how many: {roomy_count} against {cramped_count}"
        );
    }

    /// The room is the pane's, not the screen's — what is behind it was not read.
    /// Both of its dimensions, since how many words fit depends on how wide each
    /// one runs as well as on how many rows there are.
    #[test]
    fn the_room_is_the_panes_rather_than_the_screens() {
        let screen = Rect::new(0, 0, 100, 40);

        let (rows, across) = room_on(screen);

        assert!(rows > 0 && across > 0, "there is room for something");
        assert!(
            rows < usize::from(screen.height),
            "but fewer rows than the screen: {rows}"
        );
        assert!(
            across < usize::from(screen.width),
            "and narrower than it: {across}"
        );
    }

    /// It leaves what is behind it visible around the edges, which is what makes it
    /// an aside rather than another screen.
    #[test]
    fn the_pane_never_covers_the_whole_screen() {
        let screen = Rect::new(0, 0, 100, 40);

        let area = middle(screen);

        assert!(area.width < screen.width, "{area:?}");
        assert!(area.height < screen.height, "{area:?}");
        assert!(
            area.x > 0 && area.y > 0,
            "and it is not in a corner: {area:?}"
        );
    }
    /// The words of one line, for a check that reads what a row actually carries.
    fn text_of(lines: &[Line<'static>]) -> Vec<String> {
        lines.iter().map(text).collect()
    }

    /// The defect this exists for. An explanation is prose and the pane counts its
    /// own rows, so an explanation wider than the pane goes onto another row — at
    /// no width does the pane stop mid-sentence, which is the one thing a pane for
    /// explaining words cannot do.
    #[test]
    fn no_width_leaves_an_explanation_unfinished() {
        let said = "the hardlink, the indexer and the VPN";

        for across in [26, 40, 54, 82, 120, 200] {
            // Read as a terminal reads it: a grid of cells, where whatever a row
            // holds past its last column is not re-wrapped, it is not drawn.
            let screen = text_of(&explaining(said, 40, across))
                .iter()
                .map(|row| row.chars().take(across).collect::<String>())
                .collect::<Vec<String>>()
                .join(" ");
            for term in mentioned(said) {
                let missing: Vec<&str> = term
                    .short
                    .split_whitespace()
                    .filter(|word| !screen.contains(word))
                    .collect();
                assert!(
                    missing.is_empty(),
                    "at {across} columns `{}` lost {missing:?}: {screen}",
                    term.word
                );
            }
        }
    }

    /// Nothing the pane writes runs past its own edge either: a row past the last
    /// column of a grid of cells is not re-wrapped, it is not drawn.
    #[test]
    fn no_row_the_pane_writes_runs_past_its_edge() {
        for across in [26, 40, 54, 82] {
            for rows in text_of(&explaining(
                "the hardlink and the quality profile",
                40,
                across,
            )) {
                let counted = rows.chars().count();
                assert!(counted <= across, "{counted} of {across}: {rows}");
            }
        }
    }

    /// A pane that overflowed its own rows would lose the line at the bottom, which
    /// is the one saying how many words it had no room for.
    #[test]
    fn what_the_pane_draws_never_outruns_the_rows_it_was_given() {
        let said = "the hardlink, the indexer, the VPN, the ratio, the seed and the torrent";

        for rows in [0usize, 1, 2, 3, 5, 8, 13] {
            let drawn = explaining(said, rows, 40);
            assert!(drawn.len() <= rows, "{} rows of {rows}", drawn.len());
        }
    }

    /// Whatever it had no room for is still counted, however narrow the pane — a
    /// pane that quietly showed two of six would be read as there being two.
    #[test]
    fn a_narrow_pane_still_says_how_many_it_left_out() {
        let said = "the hardlink, the indexer, the VPN, the ratio, the seed and the torrent";

        let drawn = text_of(&explaining(said, 6, 40)).join(" ");

        assert!(drawn.contains("more, which"), "{drawn}");
    }

    /// A row continuing an explanation is indented to where the explanation began,
    /// so it cannot be read as another word's.
    #[test]
    fn a_row_continuing_an_explanation_starts_under_the_explanation() {
        let rows: Vec<String> = TERMS
            .iter()
            .filter(|term| term.word == "hardlink")
            .flat_map(|term| text_of(&taught(term, 40)))
            .collect();

        assert!(rows.len() > 1, "it took more than one row: {rows:?}");
        let continuing: Vec<&String> = rows.iter().skip(1).collect();
        assert!(
            continuing
                .iter()
                .all(|row| row.starts_with(&" ".repeat("hardlink".len() + 2))),
            "{continuing:?}"
        );
    }

    /// The title is the one row of the pane that cannot be given a second, so the
    /// width decides which of its names is used — and every one of them is whole.
    #[test]
    fn the_pane_is_never_called_by_half_a_name() {
        for across in 1u16..=120 {
            let title = titled(across);
            assert!(TITLES.contains(&title) || title == SHORTEST, "`{title}`");
            let fits = title.chars().count() <= usize::from(across);
            assert!(fits || title == SHORTEST, "`{title}` does not fit {across}");
        }
        assert!(titled(120).contains("any key closes"), "{}", titled(120));
        assert!(titled(30).ends_with("this screen "), "{}", titled(30));
        assert_eq!(titled(10), " words ");
    }
}
