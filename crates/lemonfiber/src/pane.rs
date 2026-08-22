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

use lemonfiber_core::glossary::{mentioned, Term};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::{Frame, Line, Span, Style};
use ratatui::style::Modifier;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

/// How wide the pane is, as a share of the screen.
const WIDTH: u16 = 70;

/// How tall, as a share, so it never covers everything behind it.
const HEIGHT: u16 = 60;

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
    let room = usize::from(area.height.saturating_sub(2));
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(explaining(showing, room)).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" the words on this screen — any key closes "),
        ),
        area,
    );
}

/// One line per word on the screen, and a word about what was left out.
///
/// Cut to what fits rather than scrolled: this is an aside, and an aside somebody
/// has to navigate has stopped being one. What is left out is counted, because a
/// pane that quietly showed four of nine would be read as there being four.
fn explaining(showing: &str, room: usize) -> Vec<Line<'static>> {
    let used = mentioned(showing);
    if used.is_empty() {
        return vec![Line::raw("Nothing on this screen needs a word explaining.")];
    }

    let explained = explained_in(showing, room);
    let shown = explained.len();
    let mut lines: Vec<Line<'static>> = explained
        .iter()
        .map(|term| {
            Line::from(vec![
                Span::styled(
                    format!("{}  ", term.word),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(term.short.to_owned()),
            ])
        })
        .collect();

    let left = used.len().saturating_sub(shown);
    if left > 0 {
        lines.push(Line::styled(
            format!("and {left} more, which `lemonfiber explain` will say"),
            Style::default().add_modifier(Modifier::DIM),
        ));
    }
    lines
}

/// The words this pane explains in full, given the room it has.
///
/// One place decides, because the loop that records what an operator opened must
/// record **exactly** what was explained to them. The ones the pane only names are
/// not explained — naming a word is how it stays findable, not how it gets taught —
/// and recording those would stop a later report explaining a word nobody ever read.
pub(crate) fn explained_in(showing: &str, room: usize) -> Vec<&'static Term> {
    let fits = room.saturating_sub(1).max(1);
    mentioned(showing).into_iter().take(fits).collect()
}

/// How much room the pane has for words, on a screen of this size.
pub(crate) fn room_on(screen: Rect) -> usize {
    usize::from(middle(screen).height.saturating_sub(2))
}

/// A box in the middle of the screen, leaving what is behind it visible around.
fn middle(screen: Rect) -> Rect {
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
    use super::{explained_in, explaining, middle, room_on, showing};
    use ratatui::layout::Rect;
    use ratatui::prelude::{Line, Span};

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
        let explained = explaining(&said, 10);

        assert_eq!(explained.len(), 1, "one word was on the screen");
        let first = explained.first().map(text).unwrap_or_default();
        assert!(first.starts_with("indexer"), "{first}");
    }

    /// A screen with none of them says so, rather than opening an empty box that
    /// reads as something having gone wrong.
    #[test]
    fn a_screen_with_no_words_to_explain_says_so() {
        let drawn = [Line::from(vec![Span::raw("everything is running")])];

        let explained = explaining(&showing(drawn.iter()), 10);

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

        let explained = explaining(&showing(drawn.iter()), 4);

        let last = explained.last().map(text).unwrap_or_default();
        assert!(last.starts_with("and 2 more"), "{last}");
    }

    /// The loop that records what was opened must record exactly what was
    /// explained — a word the pane only named has not been taught.
    #[test]
    fn only_the_words_it_explained_count_as_explained() {
        let said = "the indexer, the hardlink, the VPN, the ratio and the seed";

        let roomy = explained_in(said, 10);
        let cramped = explained_in(said, 3);

        let (roomy_count, cramped_count) = (roomy.len(), cramped.len());
        assert!(
            roomy_count > cramped_count,
            "room decides how many: {roomy_count} against {cramped_count}"
        );
        assert_eq!(cramped.len(), 2, "one line goes to the ones it only names");
        assert!(cramped.iter().all(|term| roomy.contains(term)));
    }

    /// The room is the pane's, not the screen's — what is behind it was not read.
    #[test]
    fn the_room_is_the_panes_rather_than_the_screens() {
        let screen = Rect::new(0, 0, 100, 40);

        let room = room_on(screen);

        assert!(room > 0, "there is room for something");
        assert!(
            room < usize::from(screen.height),
            "but less than the screen: {room}"
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
}
