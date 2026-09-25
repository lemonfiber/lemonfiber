//! Lines to read, and where in them the screen is.
//!
//! An answer is longer than a box over a dashboard. Every setting a stack declares
//! is dozens of lines, a trace is a season at a time, and a report cut to what fits
//! is a report whose end nobody can reach — so the box moves through the lines
//! rather than showing the first of them and counting the rest.
//!
//! Where it moves to is held as the line the box begins at, kept inside the lines
//! there are by whichever end it reaches. A box scrolled off its own end would be
//! empty over a screen that has plenty to say, and an operator who had scrolled
//! there would read that as the answer.
//!
//! Turning an answer into lines is here too, since a reading is what every answer
//! becomes: rendered by the renderer the command line reaches for the same answer,
//! so the two surfaces cannot come to say different things about one stack.

use lemonfiber_core::error::Problem;

use super::{Press, Stage, Wanted};

/// What is said where an answer is not the shape the question had.
const NOT_THE_ANSWER: &str = "This stack answered something other than what was asked of it.";

/// Lines to read, and which of them the box begins at.
pub(crate) struct Reading {
    /// The lines, in the order they were rendered.
    lines: Vec<String>,
    /// The first of them the box shows.
    at: usize,
}

impl Reading {
    /// A reading over these lines, beginning at the first.
    pub(crate) const fn of(lines: Vec<String>) -> Self {
        Self { lines, at: 0 }
    }

    /// Add a line to the end of it, leaving the box where the operator put it.
    ///
    /// Where they put it rather than at the newest line: a reading that jumped to the
    /// end every time something arrived would move out from under somebody who had
    /// scrolled back to read an earlier one, which on a walk that says a step every
    /// few seconds is most of the time they are reading it.
    pub(crate) fn put(&mut self, line: String) {
        self.lines.push(line);
    }

    /// Move one line towards the beginning, or stay where it begins.
    pub(crate) fn back(&mut self) {
        self.at = self.at.saturating_sub(1);
    }

    /// Move one line towards the end, or stay where it ends.
    ///
    /// The end is the last line rather than the one after it, so the box always has
    /// something in it.
    pub(crate) fn forward(&mut self) {
        let last = self.lines.len().saturating_sub(1);
        self.at = self.at.saturating_add(1);
        if self.at > last {
            self.at = last;
        }
    }

    /// What the box shows, and how many lines lie either side of it.
    pub(crate) fn window(&self, rows: usize) -> (Vec<&str>, usize, usize) {
        let shown: Vec<&str> = self
            .lines
            .iter()
            .skip(self.at)
            .take(rows)
            .map(String::as_str)
            .collect();
        let below = self
            .lines
            .len()
            .saturating_sub(self.at.saturating_add(shown.len()));
        (shown, self.at, below)
    }
}

/// Over what an action came to: move through it, or put it away.
///
/// Beside [`moved`] rather than in the dispatch, because it is the whole of what a
/// press over a box with nothing else in it does — and a dispatch arm carrying its
/// own copy of that is one that can come to disagree with the reading beside it.
pub(super) fn came(stage: &mut Stage, mut reading: Reading, press: &Press) -> Wanted {
    if moved(&mut reading, press) {
        *stage = Stage::Came(reading);
    }
    Wanted::Nothing
}

/// Move through a reading, saying whether the press was a move at all.
///
/// A press that is not a move puts the reading away, which is how the pane of words
/// is put away too: one shape of dismissal on a screen where several boxes open.
pub(super) fn moved(reading: &mut Reading, press: &Press) -> bool {
    match *press {
        Press::Back => reading.back(),
        Press::Forward => reading.forward(),
        _ => return false,
    }
    true
}

/// An answer that is not the shape the question had.
///
/// Every command this screen sends has one shape of answer, so nothing reaches
/// this in an ordinary run. It is said rather than shown as nothing, because a
/// screen that went quiet would read as an action that never ran.
pub(super) fn unexpected() -> Vec<String> {
    vec![NOT_THE_ANSWER.to_owned()]
}

/// A failure, in the words the command line gives for the same one.
pub(super) fn complaint(problem: &Problem) -> Vec<String> {
    lines_of(&crate::exit::reported(problem, false))
}

/// A rendered answer as the rows a screen draws.
pub(super) fn lines_of(lines: &crate::render::Lines) -> Vec<String> {
    lines.text().lines().map(str::to_owned).collect()
}

#[cfg(test)]
mod tests;
