//! The screen: where each panel goes, and what the whole frame looks like.
//!
//! Pure over a [`Snapshot`] and the space available. The terminal, the loop and
//! the keyboard are [`crate::terminal`]'s; nothing here knows a terminal exists,
//! which is what lets the whole screen be drawn and read back in a test.
//!
//! Laid out by what is available rather than by a fixed grid: a narrow terminal
//! drops to one column rather than squeezing two into a width neither fits, so it
//! degrades by carrying less at a time and never by overlapping.

mod panels;

use lemonfiber_core::dashboard::{Snapshot, Telemetry};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// The width below which two columns stop fitting.
///
/// Two panels of forty characters and their borders. Under it the screen carries
/// one column, which is less at a time and still correct — the alternative is a
/// layout that overlaps, and a corrupted screen is worse than a tall one.
const TWO_COLUMNS: u16 = 96;

/// Draw the whole screen.
pub(crate) fn draw(frame: &mut Frame, snapshot: &Snapshot) {
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
        Paragraph::new(panels::header(telemetry, &snapshot.health)),
        top,
    );
    frame.render_widget(
        Paragraph::new(Line::styled(
            "q quit   r refresh",
            Style::default().add_modifier(Modifier::DIM),
        )),
        bottom,
    );

    for (area, (title, lines)) in places(body).into_iter().zip(sections(snapshot)) {
        frame.render_widget(
            Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(title)),
            area,
        );
    }
}

/// Each panel, in the order they are read: what is wrong first, then what is
/// happening, then what it is running on.
fn sections(snapshot: &Snapshot) -> Vec<(&'static str, Vec<Line<'static>>)> {
    vec![
        ("VPN", panels::vpn(snapshot.vpn.as_ref())),
        ("Transfers", panels::transfers(&snapshot.transfers)),
        ("Queues", panels::queues(&snapshot.queue)),
        ("Storage", panels::storage(&snapshot.storage)),
        ("Services", panels::services(&snapshot.services)),
    ]
}

/// Where the five panels go in the space there is.
///
/// Two columns where the terminal is wide enough for both, one where it is not.
/// Every panel gets a place in either case: dropping one would leave an operator
/// looking for something that is simply not on the screen.
fn places(body: Rect) -> Vec<Rect> {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Ratio(1, 3); 3])
        .split(body);
    if body.width < TWO_COLUMNS {
        // One column: the same five panels, stacked, each narrower and taller.
        return Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Ratio(1, 5); 5])
            .split(body)
            .to_vec();
    }
    let mut places = Vec::new();
    for (row, count) in rows.iter().zip([2usize, 2, 1]) {
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
pub(crate) mod tests {
    use super::{draw, places, TWO_COLUMNS};
    use lemonfiber_core::dashboard::{
        Hardlink, Panel, Protocol, Reading, Snapshot, Storage, Telemetry, Transfer,
    };
    use lemonfiber_core::health::{Reach, Summary};
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::Terminal;

    /// A snapshot with every panel filled, for the tests that need one.
    pub(crate) fn a_snapshot() -> Snapshot {
        Snapshot {
            telemetry: Telemetry::Live,
            health: Summary::of(Reach::Running, &[], "1000"),
            vpn: None,
            transfers: Panel::Ready(vec![Transfer {
                name: "Some.Release".to_owned(),
                protocol: Protocol::Torrent,
                progress: 42,
                speed: Reading::Known(5_000_000),
                eta: Some(std::time::Duration::from_secs(600)),
            }]),
            queue: Panel::Ready(Vec::new()),
            storage: Panel::Ready(Storage {
                free: Reading::Known(500_000_000_000),
                exhaustion: None,
                hardlink: Hardlink::Linking,
            }),
            services: Panel::Ready(Vec::new()),
        }
    }

    /// The whole screen as text, drawn at the given size.
    fn drawn(snapshot: &Snapshot, width: u16, height: u16) -> String {
        Terminal::new(TestBackend::new(width, height))
            .ok()
            .map(|mut terminal| {
                let _ = terminal.draw(|frame| draw(frame, snapshot));
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

    #[test]
    fn the_screen_carries_every_panel_and_the_state_of_the_screen_itself() {
        let text = drawn(&a_snapshot(), 120, 40);
        for panel in ["VPN", "Transfers", "Queues", "Storage", "Services"] {
            assert!(text.contains(panel), "{panel} is missing:\n{text}");
        }
        assert!(text.contains("lemonfiber"), "{text}");
        assert!(text.contains("q quit"), "{text}");
    }

    #[test]
    fn a_narrow_terminal_carries_the_same_panels_in_one_column() {
        // Degrading by carrying less at a time, never by overlapping: a corrupted
        // screen is worse than a tall one.
        let text = drawn(&a_snapshot(), 60, 60);
        for panel in ["VPN", "Transfers", "Queues", "Storage", "Services"] {
            assert!(text.contains(panel), "{panel} is missing:\n{text}");
        }
    }

    #[test]
    fn a_panel_that_went_down_marks_the_screen_even_where_the_snapshot_said_live() {
        // The panels decide it, so the header and the panels cannot disagree — a
        // snapshot claiming `live` over a dead panel is exactly the disagreement.
        let mut snapshot = a_snapshot();
        snapshot.storage = Panel::unavailable("no data location is configured");
        let text = drawn(&snapshot, 120, 40);
        assert!(text.contains("some sources are down"), "{text}");
    }

    #[test]
    fn every_panel_has_somewhere_to_go_at_either_width() {
        // Five panels, five places. One left without a place would leave an
        // operator looking for something that is simply not on the screen.
        assert_eq!(places(Rect::new(0, 0, TWO_COLUMNS, 40)).len(), 5);
        assert_eq!(places(Rect::new(0, 0, TWO_COLUMNS - 1, 40)).len(), 5);
    }

    #[test]
    fn a_terminal_too_small_to_hold_anything_still_draws() {
        // Resizing to something absurd is a thing people do, and it must reflow
        // rather than fail.
        let text = drawn(&a_snapshot(), 8, 4);
        assert!(!text.is_empty());
    }
}
