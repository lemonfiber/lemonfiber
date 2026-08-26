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
mod tests {
    use super::{is_worth_saying, spoken, Narrating, Quiet, COLUMN, DONE};
    use lemonfiber_core::walkthrough::{Line, Narrator, Step};

    #[test]
    fn a_line_reads_as_a_step_and_the_evidence_for_it() {
        let said = spoken(&Line::searched(3, 47));
        assert!(said.contains("Searching indexers…"));
        assert!(said.ends_with("3 indexers, 47 releases"));
        assert!(said.len() > COLUMN, "the detail sits in its own column");
    }

    #[test]
    fn a_line_with_nothing_particular_to_add_is_just_the_step() {
        assert_eq!(spoken(&Line::at(Step::Downloading)), "  Downloading…");
    }

    #[test]
    fn the_ending_is_marked_rather_than_padded() {
        // An operator should be able to find the line that means it worked without
        // reading the ones above it.
        let said = spoken(&Line::saying(Step::Available, "Sintel (2010)"));
        assert!(said.contains(DONE));
        assert!(!said.contains('…'), "an ending is not in progress");
        assert!(said.contains("Sintel (2010)"));
    }

    /// The detail on these lines is a catalogue's title, and a title is written by
    /// whoever named the release.
    ///
    /// Narrated the moment it is true, so there is no report to build and nothing here
    /// passes `Lines::put` — for a while that made this the one surface drawing somebody
    /// else's text raw, and `\x1b[2J` in a title cleared the operator's screen halfway
    /// through their first walkthrough. The line is made plain at the one way out, which
    /// is what this drives.
    #[test]
    fn a_title_that_would_clear_the_screen_no_longer_can() {
        let named = "Sintel\u{1b}[2J (2010)";
        let said = crate::say::rendered(&spoken(&Line::saying(Step::Available, named)));
        assert!(!said.contains('\u{1b}'), "{said:?}");
        assert!(said.contains("Sintel"), "{said:?}");
        assert!(said.contains("(2010)"), "{said:?}");
    }

    #[test]
    fn the_choosing_line_is_kept_for_the_report_and_not_said_aloud() {
        // The operator has just typed it; saying it back is the product filling silence.
        assert!(!is_worth_saying(Step::Choosing));
        for step in Step::all().into_iter().filter(|s| *s != Step::Choosing) {
            assert!(is_worth_saying(step), "{step:?}");
        }
    }

    #[test]
    fn both_narrators_accept_a_line_and_neither_says_the_choosing_one() {
        // One prints and one does not; what matters is that a run whose answer is a
        // document has somewhere to send its narration that is not the document — and
        // that neither repeats back the thing the operator has just typed.
        for step in Step::all() {
            Narrating.said(&Line::at(step));
            Quiet.said(&Line::at(step));
        }
    }
}
