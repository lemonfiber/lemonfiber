//! What a viewer holds while it is showing logs, and what it has to admit.
//!
//! A log viewer is mostly an honesty problem. It is given more lines than it can
//! keep, from readers that can produce them faster than it can take them, while the
//! operator is looking somewhere else in the scrollback — and every one of those
//! situations has a comfortable lie available. Silently dropping the oldest lines
//! looks identical to a service that went quiet. Silently skipping lines under load
//! looks identical to a service that slowed down. Both are wrong in the direction
//! that costs the operator the line they were looking for.
//!
//! So this holds the counts rather than a flag. "Some lines were dropped" tells an
//! operator nothing they can act on; "the oldest 4,312 gave way" tells them to
//! narrow the filter or raise the bound. The three ways a line can go missing are
//! kept apart for the same reason, because they call for different answers:
//!
//! - **Truncated** — the buffer was full and the oldest gave way. Age, not speed.
//!   The answer is a bigger bound or a narrower view.
//! - **Outpaced** — the reader let lines go rather than block. Speed, not age. The
//!   answer is a narrower filter at the source, and no bound will help.
//! - **Unseen** — nothing is missing at all; the operator has scrolled away from
//!   the tail and these are waiting for them.
//!
//! Folding those into one number would tell an operator to do the wrong thing about
//! two of the three.
//!
//! Filtering happens on the way *out*, never on the way in. A viewer that discarded
//! what the current filter rejects would make every filter change destructive —
//! widening it back would show a hole rather than the lines that were always there.
//! The cost is holding lines that are not on screen, which is what the bound is for.

use std::collections::VecDeque;

use lemonfiber_ports::docker::LogLine;

use super::{declared, Level};

/// The lines a viewer is holding, and the account it owes of the ones it is not.
#[derive(Debug)]
pub struct Scrollback {
    /// The lines held, oldest first.
    held: VecDeque<LogLine>,
    /// How many it will hold before the oldest give way.
    bound: usize,
    /// Every line it has taken in, whether or not a filter shows it.
    scanned: usize,
    /// Lines the bound pushed out of the far end.
    truncated: usize,
    /// Lines a reader let go rather than block waiting to hand them over.
    outpaced: usize,
    /// Lines that have arrived since the operator scrolled away from the tail.
    unseen: usize,
    /// Whether new lines carry the view along with them.
    following: bool,
}

impl Scrollback {
    /// An empty scrollback that will hold `bound` lines, following the tail.
    ///
    /// A bound of nothing would hold nothing and report every line as truncated,
    /// which is a viewer that cannot show anything while insisting it is working —
    /// so the floor is one line.
    #[must_use]
    pub fn holding(bound: usize) -> Self {
        Self {
            held: VecDeque::new(),
            bound: bound.max(1),
            scanned: 0,
            truncated: 0,
            outpaced: 0,
            unseen: 0,
            following: true,
        }
    }

    /// Take one line in, pushing the oldest out if there is no room for it.
    ///
    /// Counted as scanned whether or not any filter will show it: the operator asked
    /// how much was looked at, not how much survived the look.
    pub fn take(&mut self, line: LogLine) {
        self.scanned += 1;
        if !self.following {
            self.unseen += 1;
        }
        self.held.push_back(line);
        let over = self.held.len().saturating_sub(self.bound);
        self.truncated += over;
        self.held.drain(..over).for_each(drop);
    }

    /// Note that a reader let `lines` go rather than block waiting to be read.
    ///
    /// Deliberately not counted as scanned. These were never looked at, and adding
    /// them to the tally would turn "how much did you read" into "how much existed",
    /// which is the one question a viewer that drops lines cannot answer.
    pub fn outpaced_by(&mut self, lines: usize) {
        self.outpaced += lines;
    }

    /// Stop following the tail, because the operator has scrolled back into history.
    pub fn detach(&mut self) {
        self.following = false;
    }

    /// Follow the tail again, having caught up on whatever arrived meanwhile.
    ///
    /// Clearing the count is the point rather than a side effect: it is a count of
    /// what the operator has not seen, and returning to the tail is them seeing it.
    pub fn follow(&mut self) {
        self.following = true;
        self.unseen = 0;
    }

    /// Whether new lines are carrying the view along with them.
    #[must_use]
    pub fn following(&self) -> bool {
        self.following
    }

    /// How many lines have arrived since the operator scrolled away from the tail.
    #[must_use]
    pub fn unseen(&self) -> usize {
        self.unseen
    }

    /// How many lines have been taken in, shown or not.
    #[must_use]
    pub fn scanned(&self) -> usize {
        self.scanned
    }

    /// How many lines the bound has pushed out of the far end.
    #[must_use]
    pub fn truncated(&self) -> usize {
        self.truncated
    }

    /// How many lines a reader let go rather than block.
    #[must_use]
    pub fn outpaced(&self) -> usize {
        self.outpaced
    }

    /// The newest lines a filter admits, up to `wanted` of them, oldest first.
    ///
    /// Read from the newest backwards and stopped as soon as there are enough, which
    /// is what keeps a screenful cheap on a full buffer: a viewer redraws on every
    /// keypress and every batch of arriving lines, and a filter that had to consider
    /// all five thousand held lines each time would do that work tens of times a
    /// second to show forty of them.
    ///
    /// Costs what it shows rather than what it holds — except when the operator has
    /// scrolled a long way back, which is the case where they are asking for an old
    /// line and the work is what the answer costs.
    #[must_use]
    pub fn latest(&self, filter: &Filter, wanted: usize) -> Vec<&LogLine> {
        let mut found: Vec<&LogLine> = self
            .held
            .iter()
            .rev()
            .filter(|line| filter.admits(line))
            .take(wanted)
            .collect();
        found.reverse();
        found
    }

    /// The held lines a filter admits, oldest first.
    #[must_use]
    pub fn showing(&self, filter: &Filter) -> Vec<&LogLine> {
        self.held
            .iter()
            .filter(|line| filter.admits(line))
            .collect()
    }
}

/// Which of the lines held are worth an operator's attention right now.
///
/// Every part is optional and they narrow together: nothing set shows everything,
/// which is what a viewer opens on.
#[derive(Debug, Default)]
pub struct Filter {
    /// The services to show; empty shows every service.
    services: Vec<String>,
    /// The least severity worth showing, where one is asked for.
    least: Option<Level>,
    /// Text a line must contain, already folded to one case.
    text: Option<String>,
}

impl Filter {
    /// Narrow to these services, or to all of them where the list is empty.
    #[must_use]
    pub fn from_services(mut self, services: &[String]) -> Self {
        self.services = services.to_vec();
        self
    }

    /// Narrow to lines that declare this severity or worse.
    #[must_use]
    pub fn at_least(mut self, least: Level) -> Self {
        self.least = Some(least);
        self
    }

    /// Narrow to lines containing this text, regardless of case.
    #[must_use]
    pub fn containing(mut self, text: &str) -> Self {
        self.text = Some(text.to_lowercase());
        self
    }

    /// Whether this line survives every part of the filter.
    #[must_use]
    pub fn admits(&self, line: &LogLine) -> bool {
        self.by_service(line) && self.by_level(line) && self.by_text(line)
    }

    /// Whether the line came from a service being shown.
    fn by_service(&self, line: &LogLine) -> bool {
        self.services.is_empty() || self.services.iter().any(|name| name == &line.service)
    }

    /// Whether the line declares a severity at least as bad as the one asked for.
    ///
    /// A line that declares none does not survive a severity filter. That is the
    /// uncomfortable half of refusing to guess: an unclassified line might be the
    /// error the operator is hunting, and it will still be hidden. Guessing instead
    /// would hide a different set — the ordinary progress this stack writes to
    /// standard error — and be wrong far more often, so the filter is honest about
    /// what it is: a search of what lines say about themselves.
    fn by_level(&self, line: &LogLine) -> bool {
        self.least
            .is_none_or(|least| declared(&line.line).is_some_and(|level| level >= least))
    }

    /// Whether the line contains the text asked for.
    fn by_text(&self, line: &LogLine) -> bool {
        self.text
            .as_ref()
            .is_none_or(|text| line.line.to_lowercase().contains(text.as_str()))
    }
}
