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
mod keys;
mod notices;

pub(crate) use keys::{wanted, Asked, Press};

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

/// What a screen is doing about the words on it.
///
/// Three states rather than two flags, because two flags can be put into a fourth
/// state that means nothing: shown, on a run that explains nothing. Written this way
/// that state cannot be reached rather than being prevented by an `&&` somebody has
/// to remember.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Words {
    /// This run explains nothing, so there is nothing for a key to open.
    Unexplained,
    /// Not shown, and a key would show them.
    Away,
    /// Shown over whatever else is on the screen.
    Shown,
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
    /// What the screen is doing about the words on it.
    words: Words,
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
            words: Words::Away,
            was: Vec::new(),
            back: 0,
            open: true,
            colours: true,
        }
    }

    /// The words this screen is showing, for the loop to record what was opened.
    ///
    /// Built from the lines the view would draw rather than from the whole
    /// scrollback: what an operator opened the pane over is what was in front of
    /// them, not everything the buffer happens to hold.
    pub(crate) fn showing_words(&self, rows: usize) -> String {
        self.showing(rows)
            .iter()
            .map(|shown| format!("{} {}", shown.service, shown.said))
            .collect::<Vec<String>>()
            .join(" ")
    }

    /// Whether the words on this screen are being shown.
    pub(crate) const fn glossary(&self) -> bool {
        matches!(self.words, Words::Shown)
    }

    /// The same viewer, explaining nothing — for a run that asked for none.
    ///
    /// A builder rather than a latch read from in here, so a test can have both
    /// kinds of viewer without settling a value that outlives it.
    pub(crate) const fn without_explanations(mut self) -> Self {
        self.words = Words::Unexplained;
        self
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

    /// What the screen is showing, and what is waiting off the top of it, in the
    /// room a title has.
    ///
    /// A title cannot be given a second row, so what it says is built to fit the one
    /// it has. The count of unseen lines is kept whatever else goes; it sits at the
    /// end, which is where a cut would take it first.
    ///
    /// Made plain here, at the end, because the names in it are a container's and not
    /// this product's. The rows in the body of the same box are made plain where they
    /// are built; a title is drawn by the border rather than by them, so it would
    /// otherwise be the one run of somebody else's text on this screen that was not.
    /// After the fitting rather than before it: what is measured is never narrower
    /// than what is drawn, and dropping a character an emulator would have obeyed
    /// only ever leaves room over.
    pub(crate) fn heading(&self, across: usize) -> String {
        let unseen = match self.held.unseen() {
            0 => String::new(),
            unseen => format!(" — {unseen} unseen"),
        };
        let room = across.saturating_sub(unseen.chars().count());
        lemonfiber_core::text::plain(&format!("{}{unseen}", self.sources(room)))
    }

    /// Which services are being shown, named while there is room to name them.
    ///
    /// As many as fit whole, and the rest counted. A name cut in half says which
    /// service no better than an absent one, and a list showing three of ten with no
    /// count reads as there being three.
    fn sources(&self, room: usize) -> String {
        match &self.service {
            Some(service) => return service.clone(),
            None if self.seen.is_empty() => return "waiting for lines".to_owned(),
            None => (),
        }
        let all = self.seen.join(", ");
        if all.chars().count() <= room {
            return all;
        }
        // Measured against the whole count rather than the number left out, which is
        // never larger — so what is built is never wider than what was measured.
        let budget = room.saturating_sub(format!(", +{} more", self.seen.len()).chars().count());
        let mut named = String::new();
        let mut counted = 0;
        for name in &self.seen {
            let next = if named.is_empty() {
                name.clone()
            } else {
                format!("{named}, {name}")
            };
            if next.chars().count() > budget {
                break;
            }
            named = next;
            counted += 1;
        }
        let left = self.seen.len() - counted;
        // Where not one name fits, the count stands on its own rather than as
        // "more" than a list that is not there.
        if named.is_empty() {
            return format!("{left} service{}", s(left));
        }
        format!("{named}, +{left} more")
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
mod tests;
