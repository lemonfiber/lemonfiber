//! The screen a live log tail is read on.
//!
//! [`lemonfiber_core::logs::viewer`] holds the lines and the account of the ones it
//! does not; what is here is the part an operator touches — where the view sits,
//! what it is narrowed to, and what a keypress means. All of it is decided here and
//! drawn in [`draw`], so that every question this screen can be asked has an answer
//! provable without a terminal.
//!
//! Two things are load-bearing and neither is obvious.
//!
//! **Where the view sits is counted from the newest line, not the oldest.** A live
//! tail is being written to at one end and truncated at the other, and an offset
//! from the start would slide under the operator every time either happened. From
//! the end, only arriving lines move it — and while the view is detached those are
//! compensated for, so a line the operator is reading stays where they left it.
//!
//! **Being detached is one fact, not two.** The offset and the scrollback's own
//! sense of whether it is following would be free to disagree, and the screen would
//! then be able to say "detached" while scrolled to the tail. So the offset is the
//! only truth and the scrollback is told, in one place, every time it changes.

use lemonfiber_core::logs::viewer::{Filter, Scrollback};
use lemonfiber_core::logs::{declared, Level};
use lemonfiber_core::plural::s;
use lemonfiber_core::ports::docker::LogLine;
use lemonfiber_core::text::plain;

pub(crate) mod draw;

/// How many lines the screen holds before the oldest give way.
///
/// Enough that scrolling back through a morning's activity works, small enough that
/// a service in a restart loop cannot eat the machine.
const HELD: usize = 5_000;

/// How many waiting lines the screen takes in one pass.
///
/// The whole of a backlog would be correct and would also stop the screen answering
/// the keyboard while it worked through it, which is the one thing a viewer of a
/// firehose must not do — an operator who cannot press a key to narrow the filter
/// has no way out of the flood.
const BATCH: usize = 500;

/// The severities the screen cycles through, in the order it offers them.
///
/// A list rather than a match on the level below, so that adding a rung is one edit
/// and there is no arm for a level nothing offers.
const RUNGS: [Option<Level>; 4] = [
    None,
    Some(Level::Info),
    Some(Level::Warn),
    Some(Level::Error),
];

/// How many of a backlog to take now, and how many to let go.
///
/// Letting some go is a deliberate trade, not a failure. Under a flood the choice is
/// between a view that shows a sample of what is happening now and one that shows
/// every line from a minute ago, and only the first is any use for watching a stack.
/// What is given up is counted and said on the screen, which is what makes it a
/// trade rather than a lie.
pub(crate) const fn sampled(waiting: usize) -> (usize, usize) {
    if waiting > BATCH {
        (BATCH, waiting - BATCH)
    } else {
        (waiting, 0)
    }
}

/// What the operator asked for.
///
/// Deliberately close to the keyboard rather than to the screen: what a key means
/// depends on whether a filter is being typed, and that is a decision this module
/// makes rather than one the terminal should be making on its behalf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Press {
    /// A character, whatever the screen is doing.
    Typed(char),
    /// Rub out the last character of whatever is being typed.
    Rubout,
    /// Finish what is being typed and apply it.
    Accept,
    /// Give up on what is being typed, or leave the screen.
    Abandon,
    /// Further back into what has already happened.
    Back,
    /// Back towards the newest line.
    Forward,
    /// All the way to the newest line.
    Tail,
}

/// One line as the screen will show it.
///
/// The severity is read once here rather than again in the drawing, so a line that
/// is filtered as a warning cannot be coloured as anything else.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Shown {
    /// Which service wrote it.
    pub(crate) service: String,
    /// What it says about its own severity, where it says anything.
    pub(crate) level: Option<Level>,
    /// The line, with anything a terminal would obey taken out.
    pub(crate) said: String,
}

/// Everything the screen knows.
pub(crate) struct Viewer {
    /// The lines, and the account of the ones that are not here.
    held: Scrollback,
    /// The one service being shown, or nothing for all of them.
    service: Option<String>,
    /// The services lines have arrived from, in the order they first did.
    seen: Vec<String>,
    /// The least severity worth showing, where one is asked for.
    least: Option<Level>,
    /// The text a line must contain, where any is asked for.
    text: Option<String>,
    /// A filter part-typed, or nothing where the operator is reading.
    typing: Option<String>,
    /// How far back from the newest admitted line the view sits.
    back: usize,
    /// Whether the operator is still here.
    open: bool,
}

impl Viewer {
    /// A screen showing everything, at the tail, with nothing in it yet.
    pub(crate) fn opened() -> Self {
        Self::holding(HELD)
    }

    /// The same, holding a stated number of lines.
    fn holding(bound: usize) -> Self {
        Self {
            held: Scrollback::holding(bound),
            service: None,
            seen: Vec::new(),
            least: None,
            text: None,
            typing: None,
            back: 0,
            open: true,
        }
    }

    /// Take one line in.
    ///
    /// While the view is detached an arriving line would push what the operator is
    /// reading up by one, so the offset grows with it and their place is kept. Only
    /// for a line this filter admits: one it hides changes nothing on the screen and
    /// compensating for it would move the view for a line nobody can see.
    pub(crate) fn take(&mut self, line: LogLine) {
        if !self.seen.iter().any(|name| name == &line.service) {
            self.seen.push(line.service.clone());
        }
        if !self.held.following() && self.filter().admits(&line) {
            self.back += 1;
        }
        self.held.take(line);
    }

    /// Note lines let go to keep the screen answering the keyboard.
    pub(crate) fn outpaced_by(&mut self, lines: usize) {
        self.held.outpaced_by(lines);
    }

    /// Whether the operator is still here.
    pub(crate) const fn open(&self) -> bool {
        self.open
    }

    /// The filter being typed, where one is.
    pub(crate) fn typing(&self) -> Option<&str> {
        self.typing.as_deref()
    }

    /// Do what a keypress asks for.
    ///
    /// What was being typed is taken out of hand here, so entry ends unless the
    /// press puts it back. That way each arm below says plainly whether it is still
    /// a search being written, rather than leaving the mode to be reasoned about.
    pub(crate) fn pressed(&mut self, press: Press) {
        match self.typing.take() {
            Some(typed) => self.while_typing(typed, press),
            None => self.while_reading(press),
        }
    }

    /// What a key means while a filter is being typed.
    ///
    /// Every printable character is text rather than a command, which is why the
    /// mode exists at all: an operator searching for `queue` should not have the `q`
    /// close the screen out from under them.
    fn while_typing(&mut self, mut typed: String, press: Press) {
        match press {
            Press::Typed(character) => {
                typed.push(character);
                self.typing = Some(typed);
            }
            Press::Rubout => {
                typed.pop();
                self.typing = Some(typed);
            }
            Press::Accept => {
                self.text = (!typed.is_empty()).then_some(typed);
                self.back_to_the_tail();
            }
            // What was typed goes and what was in force stays. Abandoning a search
            // should put the operator back where they were, not clear the filter
            // they had before they started typing a new one.
            Press::Abandon => (),
            Press::Back | Press::Forward | Press::Tail => self.typing = Some(typed),
        }
    }

    /// What a key means while the operator is reading.
    fn while_reading(&mut self, press: Press) {
        match press {
            Press::Typed('q') | Press::Abandon => self.open = false,
            Press::Typed('/') => self.typing = Some(String::new()),
            Press::Typed('f') | Press::Tail => self.back_to_the_tail(),
            Press::Typed('s') => self.next_service(),
            Press::Typed('w') => self.next_rung(),
            Press::Typed('c') => self.unfiltered(),
            Press::Typed(_) | Press::Accept | Press::Rubout => (),
            Press::Back => self.further_back(),
            Press::Forward => self.nearer_the_tail(),
        }
    }

    /// Show the next service on its own, or all of them again at the end.
    fn next_service(&mut self) {
        self.service = match &self.service {
            None => self.seen.first().cloned(),
            Some(current) => self
                .seen
                .iter()
                .position(|name| name == current)
                .and_then(|at| self.seen.get(at + 1))
                .cloned(),
        };
        self.back_to_the_tail();
    }

    /// Ask for the next severity up, or for all of them again at the top.
    fn next_rung(&mut self) {
        let at = RUNGS
            .iter()
            .position(|rung| *rung == self.least)
            .unwrap_or(0);
        self.least = RUNGS.get(at + 1).copied().flatten();
        self.back_to_the_tail();
    }

    /// Put every filter back to showing everything.
    fn unfiltered(&mut self) {
        self.service = None;
        self.least = None;
        self.text = None;
        self.back_to_the_tail();
    }

    /// Further back into what has already happened, stopping at the oldest line.
    fn further_back(&mut self) {
        let admitted = self.held.showing(&self.filter()).len();
        self.back = self.back.saturating_add(1).min(admitted.saturating_sub(1));
        self.told();
    }

    /// One line nearer the newest.
    fn nearer_the_tail(&mut self) {
        self.back = self.back.saturating_sub(1);
        self.told();
    }

    /// All the way to the newest line.
    fn back_to_the_tail(&mut self) {
        self.back = 0;
        self.told();
    }

    /// Tell the scrollback where the view now sits.
    ///
    /// The one place that happens, so the offset and what the screen says about it
    /// cannot come apart.
    fn told(&mut self) {
        if self.back == 0 {
            self.held.follow();
        } else {
            self.held.detach();
        }
    }

    /// What is in force right now.
    fn filter(&self) -> Filter {
        let mut filter = Filter::default();
        if let Some(service) = &self.service {
            filter = filter.from_services(std::slice::from_ref(service));
        }
        if let Some(least) = self.least {
            filter = filter.at_least(least);
        }
        if let Some(text) = &self.text {
            filter = filter.containing(text);
        }
        filter
    }

    /// The lines to put on a screen this many rows tall, oldest first.
    pub(crate) fn showing(&self, rows: usize) -> Vec<Shown> {
        let filter = self.filter();
        let admitted = self.held.showing(&filter);
        let end = admitted.len().saturating_sub(self.back);
        let start = end.saturating_sub(rows);
        admitted
            .get(start..end)
            .unwrap_or_default()
            .iter()
            .map(|line| Shown {
                service: plain(&line.service),
                level: declared(&line.line),
                said: plain(&line.line),
            })
            .collect()
    }

    /// What to say instead of lines, where the filter admits none.
    ///
    /// Saying how much was looked at is the point. "No matches" over an empty screen
    /// reads the same whether the filter is too narrow or nothing has arrived at all,
    /// and those call for opposite responses.
    pub(crate) fn nothing(&self) -> Option<String> {
        if self.held.showing(&self.filter()).is_empty() {
            return Some(format!(
                "nothing matches — {} lines scanned",
                self.held.scanned()
            ));
        }
        None
    }

    /// What the screen is showing, and what is waiting off the top of it.
    pub(crate) fn heading(&self) -> String {
        let sources = match &self.service {
            Some(service) => service.clone(),
            None if self.seen.is_empty() => "waiting for lines".to_owned(),
            None => self.seen.join(", "),
        };
        match self.held.unseen() {
            0 => sources,
            unseen => format!("{sources} — {unseen} unseen"),
        }
    }

    /// What is in force, and what the screen has had to give up.
    ///
    /// The two kinds of loss are said apart because they call for different answers:
    /// older lines went because the screen is only so deep, and skipped ones went
    /// because the stack is writing faster than anyone can read.
    pub(crate) fn footing(&self) -> String {
        let mut said: Vec<String> = Vec::new();
        if let Some(text) = &self.text {
            said.push(format!("/{text}"));
        }
        if let Some(least) = self.least {
            said.push(format!("{}+", least.word()));
        }
        if !self.held.following() {
            said.push("detached".to_owned());
        }
        let truncated = self.held.truncated();
        if truncated > 0 {
            said.push(format!("{truncated} older line{} dropped", s(truncated)));
        }
        let outpaced = self.held.outpaced();
        if outpaced > 0 {
            said.push(format!("{outpaced} line{} skipped to keep up", s(outpaced)));
        }
        if said.is_empty() {
            said.push("following".to_owned());
        }
        said.join(" · ")
    }
}

#[cfg(test)]
mod tests {
    use super::{sampled, Press, Shown, Viewer, BATCH};
    use lemonfiber_core::logs::Level;
    use lemonfiber_core::ports::docker::{LogLine, Stream};

    /// One line as the engine hands it over.
    fn line(service: &str, said: &str) -> LogLine {
        LogLine {
            service: service.to_owned(),
            stream: Stream::Stdout,
            at: None,
            line: said.to_owned(),
        }
    }

    /// A viewer holding everything, already fed these lines.
    fn fed(said: &[(&str, &str)]) -> Viewer {
        let mut viewer = Viewer::opened();
        for (service, text) in said {
            viewer.take(line(service, text));
        }
        viewer
    }

    /// Two services' worth of lines, for the tests that need more than one source.
    fn a_viewer() -> Viewer {
        fed(&[
            ("sonarr", "INFO Grabbed an episode"),
            ("radarr", "WARN Import timed out"),
            ("sonarr", "Torrent finished"),
        ])
    }

    /// What the screen is showing, as the words of each line.
    fn shown(viewer: &Viewer, rows: usize) -> Vec<String> {
        viewer
            .showing(rows)
            .into_iter()
            .map(|shown| shown.said)
            .collect()
    }

    /// Press each of these in turn.
    fn press(viewer: &mut Viewer, presses: &[Press]) {
        for asked in presses {
            viewer.pressed(*asked);
        }
    }

    /// Type a search and apply it.
    fn search(viewer: &mut Viewer, text: &str) {
        viewer.pressed(Press::Typed('/'));
        for character in text.chars() {
            viewer.pressed(Press::Typed(character));
        }
        viewer.pressed(Press::Accept);
    }

    /// The trade the screen makes under a flood, and the one it does not make when
    /// there is no flood to make it about.
    #[test]
    fn a_small_backlog_is_taken_whole_and_a_flood_only_in_part() {
        assert_eq!(sampled(0), (0, 0));
        assert_eq!(sampled(10), (10, 0));
        assert_eq!(sampled(BATCH), (BATCH, 0));
        assert_eq!(sampled(BATCH + 7), (BATCH, 7));
    }

    #[test]
    fn a_new_viewer_is_open_at_the_tail_with_nothing_in_it() {
        let viewer = Viewer::opened();

        assert!(viewer.open());
        assert_eq!(viewer.typing(), None);
        assert_eq!(viewer.heading(), "waiting for lines");
        assert_eq!(viewer.footing(), "following");
    }

    #[test]
    fn each_service_is_named_once_however_many_lines_it_writes() {
        assert_eq!(a_viewer().heading(), "sonarr, radarr");
    }

    /// What the screen shows about one line: who said it, how bad they said it was,
    /// and the words themselves.
    #[test]
    fn a_shown_line_carries_its_source_its_severity_and_its_words() {
        let viewer = fed(&[("radarr", "WARN Import timed out")]);

        assert_eq!(
            viewer.showing(1),
            vec![Shown {
                service: "radarr".to_owned(),
                level: Some(Level::Warn),
                said: "WARN Import timed out".to_owned(),
            }]
        );
    }

    /// A log line is text from somebody else's container, and a terminal is not a
    /// text box.
    #[test]
    fn a_line_that_would_drive_the_terminal_is_shown_without_the_instructions() {
        let viewer = fed(&[("sonarr", "INFO \u{1b}[2Jand your screen is gone")]);
        let said = shown(&viewer, 1).concat();

        assert!(!said.contains('\u{1b}'), "{said}");
        assert!(said.contains("and your screen is gone"), "{said}");
    }

    #[test]
    fn quitting_and_escaping_both_leave() {
        let mut quit = a_viewer();
        quit.pressed(Press::Typed('q'));
        assert!(!quit.open());

        let mut escaped = a_viewer();
        escaped.pressed(Press::Abandon);
        assert!(!escaped.open());
    }

    #[test]
    fn keys_the_screen_has_no_use_for_change_nothing() {
        let mut viewer = a_viewer();
        press(
            &mut viewer,
            &[Press::Typed('x'), Press::Accept, Press::Rubout],
        );

        assert!(viewer.open());
        assert_eq!(viewer.footing(), "following");
        assert_eq!(shown(&viewer, 10).len(), 3);
    }

    /// The whole reason a typing mode exists: an operator searching for `queue`
    /// must not have the `q` close the screen out from under them.
    #[test]
    fn a_search_is_typed_rubbed_out_and_applied_without_the_keys_acting() {
        let mut viewer = a_viewer();
        press(
            &mut viewer,
            &[
                Press::Typed('/'),
                Press::Typed('q'),
                Press::Typed('t'),
                Press::Typed('i'),
                Press::Typed('m'),
            ],
        );
        assert!(viewer.open(), "the q was text, not a command");
        assert_eq!(viewer.typing(), Some("qtim"));

        viewer.pressed(Press::Rubout);
        viewer.pressed(Press::Rubout);
        viewer.pressed(Press::Rubout);
        viewer.pressed(Press::Rubout);
        assert_eq!(viewer.typing(), Some(""));

        press(&mut viewer, &[Press::Typed('t'), Press::Typed('i')]);
        viewer.pressed(Press::Accept);

        assert_eq!(viewer.typing(), None);
        assert_eq!(shown(&viewer, 10), ["WARN Import timed out"]);
        let footing = viewer.footing();
        assert!(footing.contains("/ti"), "{footing}");
    }

    #[test]
    fn a_search_typed_and_left_empty_asks_for_everything() {
        let mut viewer = a_viewer();
        search(&mut viewer, "");

        assert_eq!(viewer.footing(), "following");
        assert_eq!(shown(&viewer, 10).len(), 3);
    }

    /// Giving up on a search puts the operator back where they were, rather than
    /// clearing the filter they had before they started typing a new one.
    #[test]
    fn abandoning_a_search_keeps_the_one_that_was_in_force() {
        let mut viewer = a_viewer();
        search(&mut viewer, "timed");

        press(&mut viewer, &[Press::Typed('/'), Press::Typed('z')]);
        viewer.pressed(Press::Abandon);

        assert_eq!(viewer.typing(), None);
        assert!(viewer.open(), "the escape left the search, not the screen");
        assert_eq!(shown(&viewer, 10), ["WARN Import timed out"]);
    }

    #[test]
    fn scrolling_while_a_search_is_typed_leaves_the_search_alone() {
        let mut viewer = a_viewer();
        press(&mut viewer, &[Press::Typed('/'), Press::Typed('t')]);

        press(&mut viewer, &[Press::Back, Press::Forward, Press::Tail]);

        assert_eq!(viewer.typing(), Some("t"));
        assert_eq!(viewer.footing(), "following", "and nothing scrolled");
    }

    #[test]
    fn the_severity_asked_for_cycles_up_and_back_to_everything() {
        let mut viewer = a_viewer();

        for expected in ["info+", "warn+", "error+"] {
            viewer.pressed(Press::Typed('w'));
            assert_eq!(viewer.footing(), expected);
        }
        viewer.pressed(Press::Typed('w'));
        assert_eq!(viewer.footing(), "following");
    }

    #[test]
    fn the_service_shown_cycles_through_them_and_back_to_all() {
        let mut viewer = a_viewer();

        viewer.pressed(Press::Typed('s'));
        assert_eq!(viewer.heading(), "sonarr");
        assert_eq!(shown(&viewer, 10).len(), 2);

        viewer.pressed(Press::Typed('s'));
        assert_eq!(viewer.heading(), "radarr");

        viewer.pressed(Press::Typed('s'));
        assert_eq!(viewer.heading(), "sonarr, radarr");
    }

    #[test]
    fn cycling_services_before_any_line_arrives_stays_on_all_of_them() {
        let mut viewer = Viewer::opened();
        viewer.pressed(Press::Typed('s'));

        assert_eq!(viewer.heading(), "waiting for lines");
    }

    #[test]
    fn clearing_puts_every_filter_back_at_once() {
        let mut viewer = a_viewer();
        search(&mut viewer, "timed");
        press(&mut viewer, &[Press::Typed('w'), Press::Typed('s')]);

        viewer.pressed(Press::Typed('c'));

        assert_eq!(viewer.footing(), "following");
        assert_eq!(viewer.heading(), "sonarr, radarr");
        assert_eq!(shown(&viewer, 10).len(), 3);
    }

    /// Scrolling back detaches, stops at the oldest line rather than running off
    /// the end of it, and returning to the tail attaches again.
    #[test]
    fn scrolling_back_detaches_stops_at_the_oldest_and_comes_back() {
        let mut viewer = a_viewer();

        press(&mut viewer, &[Press::Back, Press::Back, Press::Back]);
        assert_eq!(viewer.footing(), "detached");
        assert_eq!(
            shown(&viewer, 1),
            ["INFO Grabbed an episode"],
            "three presses over three lines stop at the oldest"
        );

        viewer.pressed(Press::Forward);
        assert_eq!(viewer.footing(), "detached");

        viewer.pressed(Press::Forward);
        assert_eq!(viewer.footing(), "following");
    }

    #[test]
    fn following_and_the_end_key_both_return_to_the_tail() {
        for key in [Press::Typed('f'), Press::Tail] {
            let mut viewer = a_viewer();
            viewer.pressed(Press::Back);
            assert_eq!(viewer.footing(), "detached");

            viewer.pressed(key);
            assert_eq!(viewer.footing(), "following");
        }
    }

    /// The point of counting the offset from the newest line: what the operator is
    /// reading stays where they left it while the stack keeps writing.
    #[test]
    fn a_line_arriving_while_detached_keeps_the_operators_place() {
        let mut viewer = fed(&[
            ("sonarr", "alpha timed"),
            ("sonarr", "beta timed"),
            ("sonarr", "gamma timed"),
        ]);
        search(&mut viewer, "timed");
        viewer.pressed(Press::Back);
        assert_eq!(shown(&viewer, 1), ["beta timed"]);

        // One the filter hides moves nothing, because it changes nothing on screen.
        viewer.take(line("sonarr", "delta hidden"));
        assert_eq!(shown(&viewer, 1), ["beta timed"]);

        // One it admits would push the view up by a line, so the offset grows with it.
        viewer.take(line("sonarr", "epsilon timed"));
        assert_eq!(shown(&viewer, 1), ["beta timed"]);

        let heading = viewer.heading();
        assert!(heading.contains("2 unseen"), "{heading}");
    }

    #[test]
    fn the_screen_shows_what_fits_ending_at_the_newest_line() {
        let viewer = a_viewer();

        assert_eq!(
            shown(&viewer, 2),
            ["WARN Import timed out", "Torrent finished"]
        );
        assert_eq!(shown(&viewer, 10).len(), 3, "more room than lines is fine");
        assert!(shown(&viewer, 0).is_empty());
    }

    /// An empty screen that does not say how much was looked at reads the same
    /// whether the filter is too narrow or nothing has arrived at all.
    #[test]
    fn a_filter_matching_nothing_is_stated_with_how_much_was_scanned() {
        let mut viewer = a_viewer();
        assert_eq!(viewer.nothing(), None);

        search(&mut viewer, "nothing says this");

        assert_eq!(
            viewer.nothing(),
            Some("nothing matches — 3 lines scanned".to_owned())
        );
    }

    /// The two kinds of loss are said apart because they call for different answers:
    /// a deeper buffer against one, a narrower filter against the other.
    #[test]
    fn lines_dropped_for_age_and_lines_skipped_for_speed_are_said_apart() {
        let mut viewer = Viewer::holding(2);
        for said in ["one", "two", "three"] {
            viewer.take(line("sonarr", said));
        }
        assert_eq!(viewer.footing(), "1 older line dropped");

        viewer.take(line("sonarr", "four"));
        viewer.outpaced_by(1);
        viewer.outpaced_by(8);

        assert_eq!(
            viewer.footing(),
            "2 older lines dropped · 9 lines skipped to keep up"
        );
    }

    #[test]
    fn everything_in_force_is_said_at_once() {
        // Two lines the filter admits, because scrolling back through one line has
        // nowhere to go and would leave the screen attached.
        let mut viewer = fed(&[
            ("sonarr", "WARN one timed out"),
            ("sonarr", "WARN two timed out"),
        ]);
        search(&mut viewer, "timed");
        press(&mut viewer, &[Press::Typed('w'), Press::Back]);

        assert_eq!(viewer.footing(), "/timed · info+ · detached");
    }

    #[test]
    fn a_press_says_what_it_is() {
        assert!(format!("{:?}", Press::Rubout).contains("Rubout"));
    }
}
