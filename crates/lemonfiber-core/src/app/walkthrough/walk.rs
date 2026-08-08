//! What a walk has said so far, and every way it can end.
//!
//! Two jobs, and they are the same job: every line is said the moment it is true *and*
//! kept, so that a run watched on a terminal and a run read back as JSON are the same run.
//! A walkthrough that narrated to the screen and reported something else would be two
//! accounts of one event, and the operator would have no way to tell which was true.

use super::super::targets::OpenArr;
use super::super::Ctx;
use crate::model::WalkthroughReport;
use crate::walkthrough::{
    Handover, Line, Link, Narrator, Reason, Shape, State, Step, Stopped, Suggestion,
};

/// One walk in progress: where it is being said, and everything it has said.
pub(in crate::app) struct Walk<'a> {
    /// The stack it is walking.
    pub(super) ctx: &'a Ctx,
    /// Where each line goes the moment it is true.
    narrator: &'a dyn Narrator,
    /// Every line it has said, in order.
    lines: Vec<Line>,
}

impl<'a> Walk<'a> {
    /// A walk that has said nothing yet.
    pub(super) fn new(ctx: &'a Ctx, narrator: &'a dyn Narrator) -> Self {
        Self {
            ctx,
            narrator,
            lines: Vec::new(),
        }
    }

    /// Say a line: to the operator now, and to the report afterwards.
    pub(super) fn say(&mut self, line: Line) {
        self.narrator.said(&line);
        self.lines.push(line);
    }

    /// Say a line only where it adds something the last one did not.
    ///
    /// A wait polls, and a poll that saw nothing change has nothing to say — repeating
    /// "Downloading…" every few seconds is noise that buries the lines that matter.
    pub(super) fn say_if_new(&mut self, line: Line) {
        if self.lines.last() == Some(&line) {
            return;
        }
        self.say(line);
    }

    /// The furthest step this walk has narrated.
    pub(super) fn furthest(&self) -> Step {
        self.lines
            .iter()
            .map(|line| line.step)
            .max()
            .unwrap_or_default()
    }

    /// A walk that stopped, with nothing quoted.
    pub(super) fn stopped(
        &mut self,
        shape: Shape,
        item: Option<String>,
        reason: Reason,
    ) -> WalkthroughReport {
        self.ending(shape, item, State::Failed, Some(Stopped::plain(reason)))
    }

    /// A walk that stopped, with what the services were saying attached.
    pub(super) fn stopped_quoting(
        &mut self,
        shape: Shape,
        item: Option<String>,
        reason: Reason,
        logs: Vec<String>,
    ) -> WalkthroughReport {
        self.ending(
            shape,
            item,
            State::Failed,
            Some(Stopped::quoting(reason, logs)),
        )
    }

    /// A walk that got all the way, with what the import did to the file.
    pub(super) fn finished(
        &mut self,
        shape: Shape,
        item: &str,
        link: Option<Link>,
        household: bool,
    ) -> WalkthroughReport {
        let mut report = self.ending(shape, Some(item.to_owned()), State::Complete, None);
        report.link = link;
        report.handover = Some(Handover::of(household));
        report
    }

    /// A walk whose download outlived the operator's patience and was left running.
    ///
    /// Not a failure and not a finish: the thing they were promised is still happening,
    /// and they have their terminal back. Saying which it is, is the whole value.
    pub(super) fn handed_off(&mut self, item: &str) -> WalkthroughReport {
        let mut report = self.ending(
            Shape::Pipeline,
            Some(item.to_owned()),
            State::Downloading,
            None,
        );
        report.in_background = true;
        report
    }

    /// A walk that found the stack already had what was asked for.
    ///
    /// Detected rather than acquired again, and the operator is offered something else —
    /// re-fetching what is already on disk would teach them the product does not look.
    pub(super) fn already_here(&mut self, item: &str, arrs: &[OpenArr]) -> WalkthroughReport {
        let mut report = self.ending(
            Shape::Pipeline,
            Some(item.to_owned()),
            State::Complete,
            None,
        );
        report.already_here = true;
        let kinds: Vec<crate::recyclarr::Kind> = arrs.iter().map(|arr| arr.kind).collect();
        report.suggestions = Suggestion::for_kinds(&kinds)
            .into_iter()
            .filter(|suggestion| !item.contains(suggestion.title))
            .map(|suggestion| suggestion.said())
            .collect();
        report
    }

    /// The report every ending is built from.
    fn ending(
        &mut self,
        shape: Shape,
        item: Option<String>,
        state: State,
        stopped: Option<Stopped>,
    ) -> WalkthroughReport {
        WalkthroughReport {
            shape,
            state,
            proves: shape.proves().to_owned(),
            item,
            lines: std::mem::take(&mut self.lines),
            stopped,
            link: None,
            handover: None,
            suggestions: Vec::new(),
            in_background: false,
            already_here: false,
        }
    }
}
