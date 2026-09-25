//! Saying each line the moment it is true.
//!
//! This is the whole reason the walkthrough exists rather than a report that says what
//! happened: the operator watches six services do something to a file, and afterwards
//! knows what their stack does. A line that arrives after the fact teaches nothing.
//!
//! Two narrators, because a run whose answer is a JSON document must not have prose
//! interleaved into it — a consumer parsing that stream would be handed something that is
//! not a document at all.

use crate::say::say;
use lemonfiber_core::walkthrough::{Line, Narrator, Step};

/// How wide the said-part of a line is before its detail, so the details line up in a
/// column and the eye reads down them.
const COLUMN: usize = 32;

/// The mark on the line that means it worked.
const DONE: &str = "✓";

/// A narrator that puts each line on the terminal as it arrives.
pub(crate) struct Narrating;

impl Narrator for Narrating {
    fn said(&self, line: &Line) {
        if !is_worth_saying(line.step) {
            return;
        }
        say!("{}", spoken(line));
    }
}

/// A narrator that says nothing — for a run whose whole answer is the document at the end.
pub(crate) struct Quiet;

impl Narrator for Quiet {
    fn said(&self, _line: &Line) {}
}

/// One line as it reads on a terminal: the step, padded, then what was specifically true.
///
/// The last step is marked rather than padded — it is an ending, not another thing in
/// progress, and the operator should be able to find it without reading.
pub(crate) fn spoken(line: &Line) -> String {
    if line.step.is_the_end() {
        return format!("  {DONE} {} — {}", line.said, line.detail);
    }
    let said = format!("{}…", line.said);
    if line.detail.is_empty() {
        return format!("  {said}");
    }
    format!("  {said:<COLUMN$}{}", line.detail)
}

/// Whether a step's line is worth putting on a terminal at all.
///
/// Choosing is narrated in the report but not live: at the moment it happens the operator
/// has just typed the thing, and repeating it back is the product filling silence.
pub(crate) const fn is_worth_saying(step: Step) -> bool {
    !matches!(step, Step::Choosing)
}

#[cfg(test)]
mod tests;
