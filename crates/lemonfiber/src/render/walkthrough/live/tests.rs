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
fn both_narrators_accept_every_step_a_walk_reaches() {
    // One prints and one does not; what matters is that a run whose answer is a
    // document has somewhere to send its narration that is not the document — and
    // that neither repeats back the thing the operator has just typed.
    for step in Step::all() {
        Narrating.said(&Line::at(step));
        Quiet.said(&Line::at(step));
    }
}
