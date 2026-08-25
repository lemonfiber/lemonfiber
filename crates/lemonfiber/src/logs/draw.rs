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
mod tests {
    use super::{colour, draw, widest};
    use crate::logs::{Press, Viewer};
    use lemonfiber_core::logs::{declared, Level};
    use lemonfiber_core::ports::docker::{LogLine, Stream};
    use ratatui::backend::TestBackend;
    use ratatui::style::{Color, Modifier, Style};
    use ratatui::Terminal;

    /// One line as the engine hands it over.
    fn line(service: &str, said: &str) -> LogLine {
        LogLine {
            service: service.to_owned(),
            stream: Stream::Stdout,
            at: None,
            line: said.to_owned(),
        }
    }

    /// The whole screen as text, drawn at the given size.
    fn drawn(viewer: &Viewer, width: u16, height: u16) -> String {
        Terminal::new(TestBackend::new(width, height))
            .ok()
            .map(|mut terminal| {
                let _ = terminal.draw(|frame| draw(frame, viewer));
                terminal
                    .backend()
                    .buffer()
                    .content()
                    .iter()
                    .map(ratatui::buffer::Cell::symbol)
                    .collect::<Vec<&str>>()
                    .chunks(usize::from(width))
                    .map(<[&str]>::concat)
                    .collect::<Vec<String>>()
                    .join("\n")
            })
            .unwrap_or_default()
    }

    /// A viewer with a few services' lines already in it.
    fn a_viewer() -> Viewer {
        let mut viewer = Viewer::opened();
        viewer.take(line("sonarr", "INFO Grabbed an episode"));
        viewer.take(line("radarr", "WARN Import timed out"));
        viewer.take(line("qbittorrent", "Torrent finished"));
        viewer
    }

    #[test]
    fn the_screen_carries_the_lines_their_sources_and_the_keys() {
        let screen = drawn(&a_viewer(), 80, 12);

        assert!(screen.contains("Grabbed an episode"), "{screen}");
        assert!(screen.contains("Import timed out"), "{screen}");
        assert!(screen.contains("sonarr"), "{screen}");
        assert!(screen.contains("[f] follow"), "{screen}");
        assert!(
            screen.contains("following"),
            "a screen at the tail says so: {screen}"
        );
    }

    /// The point of the requirement: an empty screen that does not say how much was
    /// looked at reads the same whether the filter is wrong or nothing has arrived.
    #[test]
    fn a_filter_that_matches_nothing_says_so_on_the_screen() {
        let mut viewer = a_viewer();
        for press in [
            Press::Typed('/'),
            Press::Typed('z'),
            Press::Typed('z'),
            Press::Accept,
        ] {
            viewer.pressed(press);
        }

        let screen = drawn(&viewer, 80, 12);
        assert!(screen.contains("nothing matches"), "{screen}");
        assert!(screen.contains("3 lines scanned"), "{screen}");
    }

    /// A search being typed replaces the standing row, so the operator can see what
    /// they are typing rather than typing blind into a screen that still says
    /// "following".
    #[test]
    fn a_search_being_typed_is_shown_as_it_is_typed() {
        let mut viewer = a_viewer();
        viewer.pressed(Press::Typed('/'));
        viewer.pressed(Press::Typed('t'));
        viewer.pressed(Press::Typed('o'));

        let screen = drawn(&viewer, 80, 12);
        assert!(screen.contains("/to"), "{screen}");
        assert!(!screen.contains("following"), "{screen}");
    }

    /// Scrolling back says so where the operator is looking.
    #[test]
    fn a_detached_screen_says_it_is_detached() {
        let mut viewer = a_viewer();
        viewer.pressed(Press::Back);

        assert!(drawn(&viewer, 80, 12).contains("detached"));
    }

    /// Every word of what a service said, wherever the screen put it.
    ///
    /// Read from the joined screen rather than row by row, since a line that wrapped
    /// is on more than one of them — which is the whole point of the check.
    fn missing<'a>(screen: &str, said: &'a str) -> Vec<&'a str> {
        said.split_whitespace()
            .filter(|word| !screen.contains(word))
            .collect()
    }

    /// The longest name in the stack this ships with, and a line to go with it.
    fn a_long_name() -> (&'static str, &'static str) {
        (
            "calibre-web-automated",
            "INFO shelved The Long Way to a Small Angry Planet",
        )
    }

    /// The requirement, on the width it fails at. A twenty-one character name and a
    /// sixty column terminal leave thirty-eight for the line, and the line is longer
    /// than that — so a screen that keeps the column beside it has to cut something,
    /// and what it cuts is the words the operator opened the viewer to read.
    #[test]
    fn a_long_name_on_a_narrow_screen_leaves_the_whole_line_readable() {
        let (service, said) = a_long_name();
        let mut viewer = Viewer::opened().without_colour();
        viewer.take(line(service, said));

        let screen = drawn(&viewer, 60, 12);

        assert!(
            screen.contains(service),
            "the name is shortened rather than moved: {screen}"
        );
        let cut = missing(&screen, said);
        assert!(cut.is_empty(), "cut off {cut:?}: {screen}");
    }

    /// The column is what moves, not what shrinks — and only where it has to. A
    /// screen with room for both keeps them on one row, which is what makes the
    /// names scannable down an edge.
    #[test]
    fn a_screen_with_room_for_both_keeps_the_name_beside_the_line() {
        let (service, said) = a_long_name();
        let mut viewer = Viewer::opened().without_colour();
        viewer.take(line(service, said));

        let beside = format!("{service} INFO shelved");
        assert!(drawn(&viewer, 100, 12).contains(&beside), "at a hundred");
        assert!(
            !drawn(&viewer, 60, 12).contains(&beside),
            "and at sixty it has moved rather than been cut"
        );
    }

    /// A line longer than the room it has is wrapped wherever the name went, so no
    /// width is one at which the screen quietly stops saying what a service said.
    #[test]
    fn no_width_cuts_a_line_short() {
        let said = "WARN the import timed out because the file was still being written";
        let mut viewer = Viewer::opened().without_colour();
        viewer.take(line("radarr", said));

        for across in [30, 40, 55, 60, 80, 100] {
            let screen = drawn(&viewer, across, 20);
            let cut = missing(&screen, said);
            assert!(
                cut.is_empty(),
                "at {across} columns, cut off {cut:?}: {screen}"
            );
        }
    }

    /// A path or a URL arrives with nothing to break at, and a run left whole runs
    /// off the edge of a grid of cells — which is not an overrun, it is a tail
    /// nobody sees.
    #[test]
    fn a_run_with_nothing_to_break_on_is_broken_rather_than_lost() {
        let mut viewer = Viewer::opened().without_colour();
        viewer.take(line(
            "qbittorrent",
            "saved /downloads/complete/Some.Release.2160p.WEB-DL.DDP5.1.H.265-GROUP/x.mkv",
        ));

        let screen = drawn(&viewer, 50, 12).replace(['\n', ' ', '│'], "");
        assert!(screen.contains("H.265-GROUP/x.mkv"), "{screen}");
    }

    /// A line is one thing: one step of the keys that page through them, and one
    /// entry on the screen. A wrapped row shown without the name above it belongs to
    /// whichever name is above it, which is another service's.
    #[test]
    fn a_line_that_will_not_fit_whole_is_left_out_rather_than_beheaded() {
        let mut viewer = Viewer::opened().without_colour();
        viewer.take(line("sonarr", "INFO grabbed"));
        viewer.take(line(
            "radarr",
            "WARN the import timed out because the file was still being written",
        ));

        // Four rows inside the borders: the newest line needs the name and two more,
        // which leaves one — and the older line needs two.
        let screen = drawn(&viewer, 46, 9);

        assert!(screen.contains("still being written"), "{screen}");
        assert!(
            !screen.contains("INFO grabbed"),
            "half of the older line was drawn without its name: {screen}"
        );
    }

    /// The exception, which is the newest line being taller than the whole body: a
    /// screen with room for none of it shows nothing at all.
    #[test]
    fn a_line_taller_than_the_screen_is_shown_as_far_as_it_goes() {
        let mut viewer = Viewer::opened().without_colour();
        viewer.take(line("sonarr", &"word ".repeat(60)));

        let screen = drawn(&viewer, 40, 7);

        assert!(screen.contains("sonarr"), "{screen}");
        assert!(screen.contains("word"), "{screen}");
    }

    /// An empty line is a line a service wrote, and an entry that came to no rows at
    /// all would take its name off the screen along with it.
    #[test]
    fn a_service_that_said_nothing_still_says_it_was_the_one_who_did() {
        let mut viewer = Viewer::opened().without_colour();
        viewer.take(line("gluetun", ""));

        assert!(drawn(&viewer, 60, 8).contains("gluetun"));
        assert!(drawn(&viewer, 30, 8).contains("gluetun"));
    }

    /// The keys are how an operator leaves a screen that is still updating without
    /// them, and `[q] quit` is the last of eight — so a row that is cut takes it
    /// first. Every key, at every width, whole.
    #[test]
    fn every_key_reaches_a_narrow_screen_whole() {
        for across in [24, 40, 60, 80, 100] {
            let screen = drawn(&a_viewer(), across, 24);
            for key in super::KEYS {
                assert!(
                    screen.contains(key),
                    "no `{key}` at {across} columns: {screen}"
                );
            }
        }
    }

    /// What the screen gave up is said at the end of the standing row, which is
    /// where a cut takes it first — and a screen that stopped saying what it dropped
    /// is making the same trade silently.
    #[test]
    fn what_the_screen_gave_up_survives_a_narrow_one() {
        let mut viewer = Viewer::opened().without_colour();
        viewer.take(line(
            "radarr",
            "WARN the import timed out because the file was still being written",
        ));
        viewer.pressed(Press::Typed('/'));
        for letter in "timed out because the file was still being wr".chars() {
            viewer.pressed(Press::Typed(letter));
        }
        viewer.pressed(Press::Accept);
        viewer.pressed(Press::Back);

        let screen = drawn(&viewer, 40, 12);
        let footing = viewer.footing();
        assert!(
            missing(&screen, &footing).is_empty(),
            "the row said `{footing}` and the screen said: {screen}"
        );
    }

    /// The title is the one row that cannot be given a second, so what it says has
    /// to fit: as many names as do, whole, and the rest counted.
    #[test]
    fn a_title_too_narrow_for_every_name_counts_the_ones_it_left_out() {
        let mut viewer = Viewer::opened().without_colour();
        for service in [
            "sonarr",
            "radarr",
            "calibre-web-automated",
            "audiobookshelf",
        ] {
            viewer.take(line(service, "INFO up"));
        }

        let screen = drawn(&viewer, 40, 12);
        assert!(screen.contains("sonarr, radarr, +2 more"), "{screen}");
    }

    /// A screen too short for its own borders asks for no lines rather than
    /// underflowing into asking for every line there is — and it draws the width it
    /// was given rather than one it worked out for itself.
    #[test]
    fn a_screen_with_no_room_shows_no_part_of_a_line() {
        let screen = drawn(&a_viewer(), 20, 1);

        assert_eq!(
            screen.chars().count(),
            20,
            "one row, twenty columns: {screen}"
        );
        assert_eq!(
            missing(&screen, "Torrent finished").len(),
            2,
            "a fragment of a line with no name against it: {screen}"
        );
    }

    /// Colour follows what the line said about itself, and a line that said nothing
    /// is left alone.
    #[test]
    fn severity_decides_colour_and_silence_decides_nothing() {
        assert_eq!(colour(Some(Level::Error), true).fg, Some(Color::Red));
        assert_eq!(colour(Some(Level::Fatal), true).fg, Some(Color::Red));
        assert_eq!(colour(Some(Level::Warn), true).fg, Some(Color::Yellow));
        assert_eq!(
            colour(Some(Level::Debug), true).add_modifier,
            Modifier::DIM,
            "detail asked for is shown as detail"
        );
        assert_eq!(colour(Some(Level::Trace), true).add_modifier, Modifier::DIM);
        assert_eq!(colour(Some(Level::Info), true), Style::default());
        assert_eq!(
            colour(None, true),
            Style::default(),
            "a line that declares no severity is not given one"
        );
    }

    /// Refused colour takes the colour and nothing else. Dimming is an attribute
    /// rather than a colour, and the convention is about colour.
    #[test]
    fn refusing_colour_leaves_the_severity_uncoloured() {
        for level in [Level::Error, Level::Fatal, Level::Warn, Level::Info] {
            assert_eq!(
                colour(Some(level), false).fg,
                None,
                "{level:?} was still coloured"
            );
        }
        assert_eq!(
            colour(Some(Level::Debug), false).add_modifier,
            Modifier::DIM,
            "dimming survives, being no colour at all"
        );
    }

    /// The point of it being safe to refuse: the severity is a word in the line, so
    /// the screen says the same thing either way.
    #[test]
    fn a_screen_without_colour_still_says_which_lines_are_bad() {
        let mut viewer = Viewer::opened().without_colour();
        viewer.take(line("radarr", "WARN Import timed out"));

        let screen = drawn(&viewer, 80, 8);
        assert!(screen.contains("WARN Import timed out"), "{screen}");
    }

    /// A cut service name no longer says which service wrote the line. Both of
    /// these are in the stack this ships with, and both lost that at twelve.
    #[test]
    fn the_column_is_wide_enough_for_every_name_in_it() {
        assert_eq!(widest(&[]), 0, "nothing seen, nothing to hold");
        assert_eq!(widest(&["sonarr".to_owned()]), 6);
        assert_eq!(
            widest(&[
                "sonarr".to_owned(),
                "calibre-web-automated".to_owned(),
                "radarr".to_owned(),
            ]),
            21,
            "the longest decides it, wherever it sits in the order"
        );
    }

    /// The whole point, end to end: a long service name reaches the screen whole.
    #[test]
    fn a_long_service_name_is_shown_in_full() {
        let mut viewer = Viewer::opened();
        viewer.take(line("calibre-web-automated", "INFO shelved something"));

        let screen = drawn(&viewer, 100, 8);
        assert!(screen.contains("calibre-web-automated"), "{screen}");
    }
    /// A flood is competing for every row, so the words are a keypress away rather
    /// than taking one of them — and they explain what is on the screen now.
    #[test]
    fn the_words_on_the_screen_are_a_keypress_away() {
        let mut viewer = Viewer::opened().without_colour();
        viewer.take(line("sonarr", "no indexer answered in time"));

        let quiet = drawn(&viewer, 90, 20);
        viewer.pressed(Press::Typed('?'));
        let asked = drawn(&viewer, 90, 20);

        assert!(
            !quiet.contains("the words on this screen"),
            "nothing until it is asked for"
        );
        assert!(asked.contains("Search engines that find"), "{asked}");

        viewer.pressed(Press::Typed('?'));
        assert!(
            !drawn(&viewer, 90, 20).contains("the words on this screen"),
            "and the same key puts it away"
        );
    }

    /// A run that asked for no explanations does not get a pane either, so the key
    /// is inert rather than opening something empty.
    #[test]
    fn a_run_that_wants_no_explanations_has_no_pane_to_open() {
        let mut viewer = Viewer::opened().without_colour().without_explanations();
        viewer.take(line("sonarr", "no indexer answered in time"));

        viewer.pressed(Press::Typed('?'));

        assert!(
            !drawn(&viewer, 90, 20).contains("the words on this screen"),
            "the key opens nothing"
        );
    }

    /// The severities, in the order they get worse.
    const EVERY: [Level; 6] = [
        Level::Trace,
        Level::Debug,
        Level::Info,
        Level::Warn,
        Level::Error,
        Level::Fatal,
    ];

    /// A line arriving on the error stream rather than the ordinary one.
    fn complained(service: &str, said: &str) -> LogLine {
        LogLine {
            stream: Stream::Stderr,
            ..line(service, said)
        }
    }

    /// No line is given a severity its own words do not say.
    ///
    /// This is what makes the colour safe to lose rather than a second thing to
    /// read: it is never the only carrier, because it is computed from words that
    /// are already on the screen. Nineteen services in this stack write ordinary
    /// progress to the error stream, so a screen that took severity from the stream
    /// would paint most of a working night red — and it would be painting something
    /// no word on the line agreed with, which is the case a reader who cannot see
    /// the colour would be left with nothing at all.
    ///
    /// Written against what the screen is given rather than against the parser, so
    /// it fails whether the guess is made in the parser or on the way to the screen.
    #[test]
    fn no_line_is_given_a_severity_its_own_words_do_not_say() {
        let mut viewer = Viewer::opened();
        viewer.take(line("radarr", "WARN Import timed out"));
        viewer.take(complained("sonarr", "Grabbed an episode"));
        viewer.take(complained("gluetun", "level=error connection refused"));
        viewer.take(line("qbittorrent", "[Warn] the tracker did not answer"));
        viewer.take(complained("sabnzbd", "queue paused"));

        for shown in viewer.showing(20) {
            assert_eq!(
                shown.level,
                declared(&shown.said),
                "a severity nothing on the line said: {}",
                shown.said
            );
        }
    }

    /// Every severity the screen paints is also a word on the screen.
    ///
    /// The other half: the words the colour was computed from have to survive to
    /// the screen, or a reader without colour is told nothing where a reader with
    /// it is told something. Each level that gets a colour at all is checked, so a
    /// colour added for a sixth level is checked the day it is added.
    #[test]
    fn every_severity_the_screen_paints_is_also_a_word_on_it() {
        for level in EVERY {
            if colour(Some(level), true).fg.is_none() {
                continue;
            }
            let mut viewer = Viewer::opened().without_colour();
            viewer.take(line(
                "radarr",
                &format!("{} something happened", level.word().to_uppercase()),
            ));

            let screen = drawn(&viewer, 100, 8).to_lowercase();
            assert!(
                screen.contains(level.word()),
                "colour is carrying {level:?} by itself: {screen}"
            );
        }
    }
    /// The pane the `?` key opens is this screen's as much as the dashboard's, and
    /// an explanation is prose: at no width does it stop mid-sentence.
    #[test]
    fn the_words_this_screen_explains_are_explained_whole() {
        let mut viewer = Viewer::opened().without_colour();
        viewer.take(line("qbittorrent", "INFO the hardlink failed"));
        viewer.pressed(Press::Typed('?'));

        let explained: Vec<&str> = lemonfiber_core::glossary::TERMS
            .iter()
            .filter(|term| term.word == "hardlink")
            .flat_map(|term| term.short.split_whitespace())
            .collect();

        for across in [60u16, 80, 120, 200] {
            let screen = drawn(&viewer, across, 24).replace('\n', " ");
            let cut: Vec<&&str> = explained
                .iter()
                .filter(|word| !screen.contains(**word))
                .collect();
            assert!(
                cut.is_empty(),
                "at {across} columns, cut off {cut:?}: {screen}"
            );
        }
    }
}
