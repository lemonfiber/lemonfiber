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
pub(crate) mod tests {
    use super::{draw, places, sections, showing, Acting, TWO_COLUMNS};
    use crate::acting::Press;
    use lemonfiber_core::app::Outcome;
    use lemonfiber_core::dashboard::{
        Hardlink, Panel, Protocol, Reading, Snapshot, Storage, Telemetry, Transfer,
    };
    use lemonfiber_core::health::{Reach, Summary};
    use lemonfiber_core::model::{FormReport, FormsReport};
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::Terminal;

    /// The transfer a filled snapshot carries, for a test that wants another.
    pub(crate) fn a_transfer() -> Transfer {
        Transfer {
            name: "Some.Release".to_owned(),
            protocol: Protocol::Torrent,
            progress: 42,
            speed: Reading::Known(5_000_000),
            eta: Some(std::time::Duration::from_secs(600)),
        }
    }

    /// A snapshot with every panel filled, for the tests that need one.
    pub(crate) fn a_snapshot() -> Snapshot {
        Snapshot {
            telemetry: Telemetry::Live,
            health: Summary::of(Reach::Running, &[], "1000"),
            vpn: None,
            transfers: Panel::Ready(vec![a_transfer()]),
            queue: Panel::Ready(Vec::new()),
            stuck: Vec::new(),
            alerts: Vec::new(),
            storage: Panel::Ready(Storage {
                free: Reading::Known(500_000_000_000),
                exhaustion: None,
                hardlink: Hardlink::Linking,
            }),
            services: Panel::Ready(Vec::new()),
            door: Panel::Ready(lemonfiber_core::model::FrontDoorReport {
                standing: lemonfiber_core::model::Standing::Established,
                chosen: lemonfiber_core::door::Chosen::Derived,
                service: Some("Seerr".to_owned()),
                address: Some(lemonfiber_core::door::Address {
                    url: "http://kitchen-nas.local:5055".to_owned(),
                    caution: None,
                }),
                facing: Some(lemonfiber_core::door::Facing::Asking),
                meaning: "send them there".to_owned(),
                beside: Vec::new(),
            }),
            household: Panel::Ready(lemonfiber_core::model::HouseholdReport {
                members: Vec::new(),
                available: true,
                findings: Vec::new(),
                filtering: None,
                policy: None,
                allows: None,
            }),
        }
    }

    /// The whole screen as text, drawn at the given size.
    fn drawn(snapshot: &Snapshot, width: u16, height: u16) -> String {
        shown(snapshot, width, height, false)
    }

    /// The loop needs these outside a frame, because it records what was opened
    /// once where drawing happens every frame.
    #[test]
    fn the_words_a_snapshot_would_show_can_be_asked_for_outside_a_frame() {
        let said = showing(&a_snapshot());

        assert!(said.contains("VPN"), "the panel titles count: {said}");
    }

    /// A full screen has no bottom to put a footnote on, so the words are a
    /// keypress away instead — and until that key, they cost the screen nothing.
    #[test]
    fn the_words_on_the_screen_are_explained_when_asked_for() {
        let snapshot = a_snapshot();

        let quiet = shown(&snapshot, 90, 30, false);
        let asked = shown(&snapshot, 90, 30, true);

        assert!(
            !quiet.contains("the words on this screen"),
            "nothing until it is asked for"
        );
        assert!(asked.contains("the words on this screen"), "{asked}");
        assert!(
            asked.contains("A tunnel your torrent"),
            "and it says what a word is for: {asked}"
        );
    }

    /// The same, with the words on the screen asked for.
    fn shown(snapshot: &Snapshot, width: u16, height: u16, glossary: bool) -> String {
        let mut acting = Acting::opened();
        if glossary {
            acting.pressed(&Press::Typed('?'));
        }
        with(snapshot, width, height, &acting)
    }

    /// The whole screen as text, with the action in whatever state it is in.
    fn with(snapshot: &Snapshot, width: u16, height: u16, acting: &Acting) -> String {
        Terminal::new(TestBackend::new(width, height))
            .ok()
            .map(|mut terminal| {
                let _ = terminal.draw(|frame| draw(frame, snapshot, acting));
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
        for (panel, _) in sections(&a_snapshot(), &[]) {
            assert!(text.contains(panel), "{panel} is missing:\n{text}");
        }
        assert!(text.contains("lemonfiber"), "{text}");
        assert!(text.contains("q quit"), "{text}");
    }

    #[test]
    fn the_screen_carries_the_address_to_hand_the_household() {
        // Asserted as the address itself rather than as a heading, because a title
        // over an empty box would satisfy a count of panels and hand nobody
        // anything — and the address is the whole of what this panel is for.
        for (width, height) in [(120, 40), (60, 90)] {
            let text = drawn(&a_snapshot(), width, height);
            assert!(text.contains("Front door"), "{text}");
            assert!(text.contains("http://kitchen-nas.local:5055"), "{text}");
        }
    }

    #[test]
    fn a_narrow_terminal_carries_the_same_panels_in_one_column() {
        // Degrading by carrying less at a time, never by overlapping: a corrupted
        // screen is worse than a tall one.
        let text = drawn(&a_snapshot(), 60, 90);
        for (panel, _) in sections(&a_snapshot(), &[]) {
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
        // A place for each panel there is, at either width. One left without a
        // place would leave an operator looking for something that is simply not
        // on the screen — so this counts against the sections themselves rather
        // than a number written twice.
        let wanted = sections(&a_snapshot(), &[]).len();
        assert_eq!(places(Rect::new(0, 0, TWO_COLUMNS, 40)).len(), wanted);
        assert_eq!(places(Rect::new(0, 0, TWO_COLUMNS - 1, 40)).len(), wanted);
    }

    /// What an action can be given is put over the panels, and the panels are still
    /// behind it — the box is a share of the screen rather than the screen.
    #[test]
    fn what_an_action_can_be_given_is_drawn_over_the_panels() {
        let snapshot = a_snapshot();
        let mut acting = Acting::opened();
        acting.pressed(&Press::Typed('d'));
        acting.told(Ok(Outcome::Forms(FormsReport {
            forms: vec![FormReport {
                id: "full".to_owned(),
                name: "Full stack".to_owned(),
                description: "everything, behind the tunnel".to_owned(),
                composable: false,
            }],
        })));

        let text = with(&snapshot, 120, 40, &acting);

        assert!(text.contains("Full stack"), "{text}");
        assert!(
            text.contains("lemonfiber"),
            "the header is still there: {text}"
        );
        assert!(text.contains("Transfers"), "a panel is still there: {text}");
    }

    /// The keys an operator can press are on the screen, or the only account of
    /// what this screen does is its source.
    #[test]
    fn the_footer_names_the_actions_this_screen_offers() {
        let text = drawn(&a_snapshot(), 200, 40);

        for hint in [
            "q quit",
            "r refresh",
            "? words",
            "ask",
            "start",
            "stop",
            "restart",
        ] {
            assert!(text.contains(hint), "{hint} is missing:\n{text}");
        }
    }

    /// The list of what this stack can be asked is drawn over the panels too, and
    /// the panels are still behind it — one box, in one place, whichever key opened
    /// it.
    #[test]
    fn the_questions_are_drawn_over_the_panels_the_way_an_action_is() {
        let snapshot = a_snapshot();
        let mut acting = Acting::opened();
        acting.pressed(&Press::Typed(crate::acting::ASK));

        let text = with(&snapshot, 120, 40, &acting);

        assert!(text.contains("versions"), "{text}");
        assert!(text.contains("Transfers"), "a panel is still there: {text}");
    }

    /// A terminal too small for the box still draws, with something open on it.
    #[test]
    fn a_terminal_too_small_for_the_box_still_draws_what_is_open_on_it() {
        let snapshot = a_snapshot();
        let mut acting = Acting::opened();
        acting.pressed(&Press::Typed(crate::acting::ASK));

        for (across, down) in [(8_u16, 4_u16), (24, 10), (40, 12)] {
            assert!(!with(&snapshot, across, down, &acting).is_empty());
        }
    }

    #[test]
    fn a_terminal_too_small_to_hold_anything_still_draws() {
        // Resizing to something absurd is a thing people do, and it must reflow
        // rather than fail.
        let text = drawn(&a_snapshot(), 8, 4);
        assert!(!text.is_empty());
    }
    /// A value long enough to need shortening at every width this is drawn at,
    /// with an end that says which value it was.
    fn a_long(mark: u8) -> String {
        format!("<<{mark}.The.Long.Way.to.a.Small.Angry.Planet.2024.2160p.WEB-DL.{mark}>>")
    }

    /// The values a wordy snapshot puts on the screen: which panel each one is in,
    /// and the mark its two ends are told apart by.
    const MARKED: [(&str, u8); 4] = [
        ("the transfer", 1),
        ("the queue", 2),
        ("the service", 3),
        ("the alert", 4),
    ];

    /// A snapshot whose every panel carries a value from somewhere else, each long
    /// enough that the screen has to do something about it.
    fn a_wordy_snapshot() -> Snapshot {
        let mut snapshot = a_snapshot();
        snapshot.transfers = Panel::Ready(vec![Transfer {
            name: a_long(1),
            ..a_transfer()
        }]);
        snapshot.queue = Panel::Ready(vec![lemonfiber_core::dashboard::Queue {
            service: a_long(2),
            depth: 4,
            stuck: 1,
        }]);
        snapshot.services = Panel::Ready(vec![lemonfiber_core::docker::Service {
            id: a_long(3),
            name: "Sonarr".to_owned(),
            profile: "tv".to_owned(),
            state: lemonfiber_core::docker::State::Running,
            criticality: lemonfiber_core::docker::Criticality::Core,
            depends_on: Vec::new(),
            exit: None,
        }]);
        snapshot.alerts = vec![lemonfiber_core::alert::Alert {
            check: "service.sonarr".to_owned(),
            kind: "service.down".to_owned(),
            moment: lemonfiber_core::alert::Moment::Onset,
            severity: lemonfiber_core::error::Severity::Warning,
            summary: a_long(4),
            remedies: vec!["start it".to_owned()],
            affected: vec!["service.sonarr".to_owned()],
        }];
        snapshot
    }

    /// Both ends of a value, which is what tells one from the next.
    fn ends(value: &str) -> (String, String) {
        let counted = value.chars().count();
        (
            value.chars().take(5).collect(),
            value.chars().skip(counted.saturating_sub(5)).collect(),
        )
    }

    /// The requirement, at the widths it failed at. A value cut at its end is a
    /// value two of which read alike: the resolution, the encoding and the group
    /// all live at the end of a release name, and a panel listing what is
    /// downloading that cannot tell two downloads apart fails at the one question
    /// it exists to answer.
    #[test]
    fn no_width_cuts_a_value_at_its_end() {
        let snapshot = a_wordy_snapshot();

        for width in [60u16, 96, 120, 160, 200] {
            let screen = drawn(&snapshot, width, 44);
            for (panel, mark) in MARKED {
                let (head, tail) = ends(&a_long(mark));
                assert!(
                    screen.contains(&head),
                    "{panel} lost its head at {width}:\n{screen}"
                );
                assert!(
                    screen.contains(&tail),
                    "{panel} lost its tail at {width}:\n{screen}"
                );
            }
        }
    }

    /// What was left out is marked, so nobody reads a shortened value as a whole
    /// one — and the marker is full stops rather than a character a terminal may
    /// not have.
    #[test]
    fn a_shortened_value_says_it_was_shortened() {
        let screen = drawn(&a_wordy_snapshot(), 120, 44);

        assert!(screen.contains("..."), "{screen}");
        assert!(!screen.contains('…'), "{screen}");
    }

    /// A value that fits is left exactly as it is: shortening one that needed no
    /// shortening would be inventing a change to it.
    #[test]
    fn a_value_that_fits_is_drawn_whole_and_unmarked() {
        let screen = drawn(&a_snapshot(), 160, 44);

        assert!(screen.contains("Some.Release"), "{screen}");
        assert!(!screen.contains("..."), "{screen}");
    }

    /// A reason a panel could not be filled is another service's words, and the
    /// end of it is commonly the part that says what to do.
    #[test]
    fn the_reason_a_panel_is_down_keeps_both_of_its_ends() {
        let mut snapshot = a_snapshot();
        let reason = a_long(5);
        snapshot.storage = Panel::unavailable(&reason);

        let screen = drawn(&snapshot, 120, 44);

        let (head, tail) = ends(&reason);
        assert!(screen.contains("unavailable"), "{screen}");
        assert!(screen.contains(&head) && screen.contains(&tail), "{screen}");
    }

    /// The pane is the log viewer's as well as this screen's, so an explanation
    /// broken onto another row here is one broken there too.
    ///
    /// A hundred and twenty columns is the width it failed at: the pane is seven
    /// tenths of the screen, which left eighty-two for a definition longer than
    /// that, and every one of them stopped mid-word.
    #[test]
    fn no_width_leaves_an_explanation_on_this_screen_unfinished() {
        let snapshot = a_snapshot();
        let explained: Vec<&str> = lemonfiber_core::glossary::TERMS
            .iter()
            .filter(|term| term.word == "hardlink")
            .flat_map(|term| term.short.split_whitespace())
            .collect();

        for width in [60u16, 96, 120, 200] {
            let screen = shown(&snapshot, width, 44, true).replace('\n', " ");
            let missing: Vec<&&str> = explained
                .iter()
                .filter(|word| !screen.contains(**word))
                .collect();
            assert!(
                missing.is_empty(),
                "at {width} columns the pane lost {missing:?}:\n{screen}"
            );
        }
    }
}
