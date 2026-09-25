use super::{
    explained_in, explaining, middle, room_on, showing, taught, taught_on, titled, SHORTEST, TITLES,
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
