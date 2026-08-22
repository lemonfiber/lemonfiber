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

use lemonfiber_core::bundle::{prose, Marks, Terms};
use lemonfiber_core::logs::viewer::{Filter, Scrollback};
use lemonfiber_core::logs::{declared, Level};
use lemonfiber_core::plural::s;
use lemonfiber_core::ports::docker::{Lifecycle, LogLine};
use lemonfiber_core::text::plain;

use notices::{noticed, remark, SELF};

pub(crate) mod draw;
mod notices;

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

/// What a keypress asked for that the screen cannot do by itself.
///
/// Writing a file is the loop's to do, not the screen's — everything else here is
/// decided without touching anything outside this module, and an export would be the
/// one exception. So it is asked for rather than done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Asked {
    /// Nothing beyond what has already been done.
    Nothing,
    /// Write the view out.
    Export,
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
    /// What each service was doing when the engine was last asked.
    ///
    /// Empty until the first look, which is what stops a viewer opening onto a
    /// notice for every service in the stack: there is nothing to have changed from.
    was: Vec<(String, Lifecycle)>,
    /// How far back from the newest admitted line the view sits.
    back: usize,
    /// Whether the operator is still here.
    open: bool,
    /// Whether colour may be added, which `NO_COLOR` can refuse.
    colours: bool,
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
            was: Vec::new(),
            back: 0,
            open: true,
            colours: true,
        }
    }

    /// The same viewer, adding no colour to what it shows.
    ///
    /// A builder rather than an argument to `opened`, so the ordinary case stays the
    /// short one and the tests that do not care about colour do not have to say so.
    pub(crate) const fn without_colour(mut self) -> Self {
        self.colours = false;
        self
    }

    /// Whether colour may be added to what this shows.
    pub(crate) const fn colours(&self) -> bool {
        self.colours
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

    /// Take the engine's account of what each service is doing.
    ///
    /// A service whose state has changed since the last look gets a line where it
    /// happened, in the stream rather than in a banner — a banner saying a service
    /// restarted cannot say *when*, and when is the whole of what makes it useful
    /// beside the lines around it.
    ///
    /// The view is not disturbed. A restart is something to notice while reading,
    /// not a reason to be thrown back to the tail, so the notice arrives the way any
    /// other line does and the operator stays where they were.
    pub(crate) fn doing(&mut self, now: &[(String, Lifecycle)]) {
        for (service, lifecycle) in now {
            let before = self
                .was
                .iter()
                .find(|(named, _)| named == service)
                .map(|(_, was)| *was);
            if before.is_some_and(|before| before != *lifecycle) {
                self.take(noticed(service, *lifecycle));
            }
        }
        self.was = now.to_vec();
    }

    /// Note lines let go to keep the screen answering the keyboard.
    pub(crate) fn outpaced_by(&mut self, lines: usize) {
        self.held.outpaced_by(lines);
    }

    /// Whether the operator is still here.
    pub(crate) const fn open(&self) -> bool {
        self.open
    }

    /// The services lines have arrived from, in the order they first did.
    pub(crate) fn seen(&self) -> &[String] {
        &self.seen
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
    pub(crate) fn pressed(&mut self, press: Press) -> Asked {
        match self.typing.take() {
            Some(typed) => {
                self.while_typing(typed, press);
                Asked::Nothing
            }
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
    fn while_reading(&mut self, press: Press) -> Asked {
        match press {
            // The one key the screen cannot answer on its own.
            Press::Typed('e') => return Asked::Export,
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
        Asked::Nothing
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
        // Bounded by the oldest admitted line, which needs one more than the offset
        // already reached rather than a count of everything the filter allows.
        let reachable = self.held.latest(&self.filter(), self.back + 2).len();
        self.back = self.back.saturating_add(1).min(reachable.saturating_sub(1));
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
        // Only as many as the screen and the offset between them can account for,
        // read from the newest backwards — a redraw costs what it shows rather than
        // what the buffer holds.
        let filter = self.filter();
        let admitted = self.held.latest(&filter, rows.saturating_add(self.back));
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

    /// The view as text, redacted, ready to be written out.
    ///
    /// Through the support bundle's own redaction rather than a second set of rules.
    /// An exported log is the same kind of thing a bundle carries — somebody else's
    /// copy of what this stack said — and two redactors would be two chances to
    /// disagree about what a credential looks like.
    ///
    /// Redacted here rather than by whatever writes the file, so that what this
    /// returns is the thing that lands on disk and a test can say so. A redaction
    /// applied on the way out of the module would be a rule nothing could check.
    ///
    /// On the bundle's default terms, which are its most careful ones: the viewer has
    /// no record of what the operator agreed to reveal, and an export is read by
    /// whoever it was sent to.
    pub(crate) fn exported(&self, marks: &Marks) -> String {
        // `prose` rejoins the lines it split, which leaves the last one bare. A file
        // that does not end in a newline is one that reads as truncated.
        let mut said = prose(&self.as_text(), marks, &Terms::default());
        said.push('\n');
        said
    }

    /// The view as text, before redaction.
    ///
    /// What the filter admits rather than everything held: an export is a copy of
    /// what the operator is looking at, and one that quietly carried the lines they
    /// had narrowed away would be a different document from the one they asked for.
    ///
    /// Tagged `service | line`, which is the shape the support bundle's own log
    /// extract takes — the redaction that runs over this was written against that
    /// shape, and a different one would be redacted differently.
    fn as_text(&self) -> String {
        self.held
            .showing(&self.filter())
            .into_iter()
            .fold(String::new(), |mut text, line| {
                text.push_str(&plain(&line.service));
                text.push_str(" | ");
                text.push_str(&plain(&line.line));
                text.push('\n');
                text
            })
    }

    /// Put a line of the viewer's own into the stream.
    ///
    /// In the stream rather than in a status row, for the same reason a restart is:
    /// what the viewer did belongs where the operator was reading, at the point it
    /// happened, and a row that is overwritten by the next thing cannot say when.
    pub(crate) fn remarked(&mut self, said: &str) {
        self.take(remark(SELF, said));
    }

    /// What to say instead of lines, where the filter admits none.
    ///
    /// Saying how much was looked at is the point. "No matches" over an empty screen
    /// reads the same whether the filter is too narrow or nothing has arrived at all,
    /// and those call for opposite responses.
    pub(crate) fn nothing(&self) -> Option<String> {
        // One line is enough to know there is something; asking for the whole
        // admitted set would scan the buffer to answer a yes-or-no question.
        if self.held.latest(&self.filter(), 1).is_empty() {
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

/// Whether colour may be added to output, given what `NO_COLOR` holds.
///
/// The convention is the variable's **presence**, not its value: set to anything at
/// all — except the empty string — and colour is refused, whatever it says. That is
/// deliberately not a flag to parse, so `NO_COLOR=0` refuses colour like everything
/// else does, which surprises people exactly once and is what every other tool does.
pub(crate) fn colours(no_color: Option<&str>) -> bool {
    no_color.is_none_or(str::is_empty)
}

#[cfg(test)]
mod tests {
    use super::{colours, sampled, Asked, Press, Shown, Viewer, BATCH};
    use lemonfiber_core::bundle::Marks;
    use lemonfiber_core::logs::Level;
    use lemonfiber_core::ports::docker::{Lifecycle, LogLine, Stream};

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

    /// Randomness a test chose, so an export reads the same on every run.
    ///
    /// Written here rather than borrowed from the fixtures crate, which this binary
    /// does not depend on: the port has one method, and this calls it.
    struct Chosen;

    impl lemonfiber_core::ports::random::Random for Chosen {
        fn bytes(&self, count: usize) -> Option<Vec<u8>> {
            Some(vec![7; count])
        }
    }

    /// The view, redacted — empty where the chosen randomness somehow refused, which
    /// every caller rules out by asserting on what it got back.
    fn exported(viewer: &Viewer) -> String {
        Marks::new(&Chosen)
            .map(|marks| viewer.exported(&marks))
            .unwrap_or_default()
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

    /// What the engine reports for one service.
    fn engine(service: &str, lifecycle: Lifecycle) -> Vec<(String, Lifecycle)> {
        vec![(service.to_owned(), lifecycle)]
    }

    /// Opening a viewer onto a running stack would otherwise print a notice for
    /// every service in it, none of which is news.
    #[test]
    fn the_first_look_at_the_engine_is_not_news() {
        let mut viewer = a_viewer();

        viewer.doing(&engine("sonarr", Lifecycle::Running));

        assert_eq!(shown(&viewer, 10).len(), 3, "nothing was added");
    }

    /// The requirement in one test: the restart is in the stream, and the view
    /// carries on around it.
    #[test]
    fn a_service_that_restarts_is_noted_without_ending_the_view() {
        let mut viewer = a_viewer();
        viewer.doing(&engine("sonarr", Lifecycle::Running));

        viewer.doing(&engine("sonarr", Lifecycle::Restarting));

        let said = shown(&viewer, 10);
        assert!(
            said.iter()
                .any(|line| line.contains("sonarr is restarting")),
            "{said:?}"
        );
        assert_eq!(
            said.len(),
            4,
            "it joined the lines rather than replacing them"
        );
        assert!(viewer.open(), "the view did not end");
        assert_eq!(viewer.footing(), "following", "and was not disturbed");
    }

    #[test]
    fn a_service_that_has_not_changed_is_not_mentioned_again() {
        let mut viewer = a_viewer();
        for _ in 0..4 {
            viewer.doing(&engine("sonarr", Lifecycle::Running));
        }

        assert_eq!(shown(&viewer, 10).len(), 3);
    }

    /// A notice is tagged with the service it is about, so narrowing to that
    /// service keeps the reason its output stopped.
    #[test]
    fn a_notice_belongs_to_the_service_it_is_about() {
        let mut viewer = a_viewer();
        viewer.doing(&engine("sonarr", Lifecycle::Running));
        viewer.doing(&engine("sonarr", Lifecycle::Restarting));

        viewer.pressed(Press::Typed('s'));

        assert_eq!(viewer.heading(), "sonarr");
        assert!(
            shown(&viewer, 10)
                .iter()
                .any(|line| line.contains("is restarting")),
            "a notice narrowed away with its own service"
        );
    }

    /// Every state the engine can report says what happened rather than naming
    /// itself: `Exited` is a state, "has stopped" is news.
    #[test]
    fn every_state_the_engine_reports_reads_as_news() {
        for (lifecycle, expected) in [
            (Lifecycle::Created, "was created"),
            (Lifecycle::Running, "is running again"),
            (Lifecycle::Paused, "was paused"),
            (Lifecycle::Restarting, "is restarting"),
            (Lifecycle::Exited, "has stopped"),
            (Lifecycle::Removing, "is being removed"),
            (Lifecycle::Dead, "died"),
        ] {
            // Seeded with something this case is not, so every one is a change.
            let seed = match lifecycle {
                Lifecycle::Running => Lifecycle::Exited,
                _ => Lifecycle::Running,
            };
            let mut viewer = a_viewer();
            viewer.doing(&engine("sonarr", seed));
            viewer.doing(&engine("sonarr", lifecycle));

            let said = shown(&viewer, 10).concat();
            assert!(said.contains(expected), "{lifecycle:?}: {said}");
        }
    }

    /// Writing a file is the loop's to do, so the screen asks rather than does.
    #[test]
    fn asking_to_export_is_reported_rather_than_carried_out() {
        let mut viewer = a_viewer();

        assert_eq!(viewer.pressed(Press::Typed('e')), Asked::Export);
        assert_eq!(viewer.pressed(Press::Typed('f')), Asked::Nothing);
        assert_eq!(
            shown(&viewer, 10).len(),
            3,
            "asking changed nothing on the screen"
        );
    }

    /// The mode exists so that letters are letters; `e` is no more special than `q`.
    #[test]
    fn an_e_typed_into_a_search_is_a_letter_not_an_export() {
        let mut viewer = a_viewer();
        viewer.pressed(Press::Typed('/'));

        assert_eq!(viewer.pressed(Press::Typed('e')), Asked::Nothing);
        assert_eq!(viewer.typing(), Some("e"));
    }

    /// An export is a copy of what the operator is looking at. One that quietly
    /// carried the lines they had narrowed away would be a different document.
    #[test]
    fn an_export_carries_what_the_filter_admits_and_nothing_else() {
        let mut viewer = a_viewer();
        search(&mut viewer, "timed");

        assert_eq!(exported(&viewer), "radarr | WARN Import timed out\n");
    }

    /// The shape the support bundle's own log extract takes, because the redaction
    /// that runs over this was written against that shape.
    #[test]
    fn an_export_tags_every_line_with_the_service_that_wrote_it() {
        let text = exported(&a_viewer());

        assert_eq!(
            text,
            "sonarr | INFO Grabbed an episode\n\
             radarr | WARN Import timed out\n\
             sonarr | Torrent finished\n"
        );
    }

    /// A log line is somebody else's text, and a file it is written into is opened
    /// by something eventually.
    #[test]
    fn an_export_carries_no_instruction_a_terminal_would_obey() {
        let viewer = fed(&[("sonarr", "INFO \u{1b}[2Jgone")]);

        let text = exported(&viewer);
        assert!(!text.contains('\u{1b}'), "{text:?}");
        assert!(text.contains("gone"), "{text:?}");
    }

    /// The requirement: an export is redacted by the support bundle's own rules, not
    /// by a second set that could disagree with them about what a credential is.
    ///
    /// Anchored on the rule that a query string goes wholesale — that is where the key
    /// nobody spotted actually lives, riding inside something that looks like an
    /// address — so this fails if the redaction is skipped or swapped for another.
    #[test]
    fn an_export_is_redacted_the_way_the_support_bundle_is() {
        let viewer = fed(&[("sonarr", "GET /api/v3/series?apikey=letmein done")]);

        let text = exported(&viewer);

        assert!(!text.contains("letmein"), "the key survived: {text}");
        assert!(
            text.contains("/api/v3/series?"),
            "the address it rode in on did not: {text}"
        );
        assert!(
            text.contains("done"),
            "and the rest of the line stays: {text}"
        );
    }

    /// What the viewer did belongs where the operator was reading, at the point it
    /// happened — a row that the next thing overwrites cannot say when.
    #[test]
    fn a_remark_joins_the_stream_under_the_viewers_own_name() {
        let mut viewer = a_viewer();

        viewer.remarked("written to somewhere.txt");

        let said = shown(&viewer, 10);
        assert!(
            said.iter()
                .any(|line| line.contains("written to somewhere.txt")),
            "{said:?}"
        );
        assert_eq!(
            said.len(),
            4,
            "it joined the lines rather than replacing them"
        );
        assert_eq!(viewer.heading(), "sonarr, radarr, lemonfiber");
    }

    /// The convention is the variable's presence, not its value — so `NO_COLOR=0`
    /// refuses colour like everything else does. Surprising exactly once, and what
    /// every other tool that honours it does.
    #[test]
    fn any_value_at_all_refuses_colour() {
        assert!(colours(None), "unset means colour is fine");
        assert!(
            colours(Some("")),
            "set but empty is not set, by the convention"
        );

        for said in ["1", "0", "true", "false", "no", " "] {
            assert!(
                !colours(Some(said)),
                "NO_COLOR={said:?} should refuse colour"
            );
        }
    }

    #[test]
    fn a_viewer_may_be_asked_to_add_no_colour() {
        assert!(Viewer::opened().colours(), "colour by default");
        assert!(!Viewer::opened().without_colour().colours());
    }
}
