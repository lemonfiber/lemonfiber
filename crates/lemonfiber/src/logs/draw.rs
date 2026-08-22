//! The log viewer as it appears.
//!
//! Nothing here decides anything. Every question — what is shown, what is in force,
//! what the screen has had to give up — is answered by [`super::Viewer`], and what
//! is here only puts those answers where they can be read. The one judgement it
//! does make is colour, and it makes it from the severity a line declared rather
//! than from the stream it arrived on: this stack writes ordinary progress to
//! standard error, and a screen that paints all of that red teaches an operator to
//! ignore red.

use lemonfiber_core::logs::Level;
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
const KEYS: &str =
    "[/] filter  [w] severity  [s] service  [c] clear  [e] export  [f] follow  [q] quit";

/// Draw the whole screen.
pub(crate) fn draw(frame: &mut Frame, viewer: &Viewer) {
    let [body, bottom, legend] = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());

    // Two rows of the body are its own border, and asking for more lines than fit
    // would silently drop the newest — which on a tail is the ones being watched.
    let rows = usize::from(body.height.saturating_sub(2));
    frame.render_widget(
        Paragraph::new(lines(viewer, rows)).block(
            Block::default()
                .borders(Borders::ALL)
                .title(Line::from(viewer.heading())),
        ),
        body,
    );
    frame.render_widget(Paragraph::new(standing(viewer)), bottom);
    frame.render_widget(
        Paragraph::new(Line::styled(
            KEYS,
            Style::default().add_modifier(Modifier::DIM),
        )),
        legend,
    );
}

/// The body: the lines, or the reason there are none.
fn lines(viewer: &Viewer, rows: usize) -> Vec<Line<'static>> {
    match viewer.nothing() {
        Some(reason) => vec![Line::styled(
            reason,
            Style::default().add_modifier(Modifier::DIM),
        )],
        None => {
            let named = widest(viewer.seen());
            viewer
                .showing(rows)
                .iter()
                .map(|shown| said(shown, viewer.colours(), named))
                .collect()
        }
    }
}

/// One line: who said it, and what they said.
///
/// The service column is padded to the width the caller measured and never cut: a
/// name cut short no longer says which service wrote the line, which is the one
/// thing the column is there for.
fn said(shown: &Shown, colours: bool, named: usize) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{:<width$} ", shown.service, width = named),
            Style::default().add_modifier(Modifier::DIM),
        ),
        Span::styled(shown.said.clone(), colour(shown.level, colours)),
    ])
}

/// The row that says what is in force and what has been given up — or, while a
/// search is being typed, the search itself.
fn standing(viewer: &Viewer) -> Line<'static> {
    match viewer.typing() {
        Some(typed) => Line::from(format!("/{typed}_")),
        None => Line::styled(
            viewer.footing(),
            Style::default().add_modifier(Modifier::DIM),
        ),
    }
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
        Some(Level::Trace | Level::Debug) => Style::default().add_modifier(Modifier::DIM),
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
/// runs rather than shifting under whoever is reading it.
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
    use lemonfiber_core::logs::Level;
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

    /// A screen too short for its own borders asks for no lines rather than
    /// underflowing into asking for every line there is.
    #[test]
    fn a_screen_with_no_room_draws_without_panicking() {
        assert!(!drawn(&a_viewer(), 20, 1).is_empty());
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
}
