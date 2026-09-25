use super::{
    asked, complained, emitted, folded, for_a_parser, refused, rendered, said, settle,
    settle_audience, shown, unicode,
};

/// Everything printed for a person is made plain here, and not by whoever asked.
///
/// Two callers had no report to build and so passed nothing through `Lines::put`:
/// Compose's own stdout, printed line by line as an image pulls or a stack starts,
/// and the walkthrough's live narration, whose detail is a catalogue's title. Both
/// are somebody else's text, and `\x1b[2J` in either clears the operator's terminal.
#[test]
fn somebody_elses_text_is_made_plain_on_its_way_out() {
    // Compose's own line, as `engine::emit_line` indents it.
    let composed = rendered("  sonarr Pulling fs layer\u{1b}[2J");
    assert!(!composed.contains('\u{1b}'), "{composed:?}");
    assert!(composed.contains("Pulling fs layer"), "{composed:?}");
    // And the shapes a terminal obeys without being a control character at all.
    for hidden in ['\u{202e}', '\u{200b}', '\u{feff}'] {
        let name = rendered(&format!("  Some{hidden}Release"));
        assert_eq!(name, "  SomeRelease", "U+{:04X}", u32::from(hidden));
    }
}

/// The breaks a caller writes into one call are ours and survive.
///
/// A line feed is the first thing a plain-text rule takes, and half the questions
/// setup asks are written with a blank line in front of them. Made plain line by
/// line, so the spacing somebody wrote stays and the escape somebody else sent goes.
#[test]
fn the_blank_line_a_caller_asked_for_is_still_there() {
    assert_eq!(
        rendered("\nWhat would you like to do?"),
        "\nWhat would you like to do?"
    );
    assert_eq!(rendered("first\n\nthird"), "first\n\nthird");
    assert_eq!(rendered("trailing\n"), "trailing\n");
    assert_eq!(rendered("a\u{1b}[2Jb\nc\rd"), "a[2Jb\ncd");
}

/// A locale that names a charset has told us what it can do; one that is unset
/// has told us nothing, and the requirement asks for a fallback where Unicode
/// is unsupported rather than wherever it is unproven.
#[test]
fn a_locale_is_believed_only_where_it_says_something() {
    for said in ["en_GB.UTF-8", "C.UTF-8", "en_US.utf8", "nl_NL.UTF-8"] {
        assert!(unicode(Some(said)), "{said} names UTF-8");
    }
    for said in ["C", "POSIX", "en_US.ISO-8859-1", "ja_JP.eucJP"] {
        assert!(!unicode(Some(said)), "{said} says it cannot");
    }
    assert!(unicode(None), "nothing said is not an objection");
    assert!(unicode(Some("")), "and neither is an empty answer");
    assert!(
        unicode(Some("en_GB")),
        "a locale naming no charset has not refused"
    );
}

/// The marks are the point: six verdicts that folded to the same character
/// would be worse than six that look plain.
#[test]
fn every_mark_folds_to_something_of_its_own() {
    let marks = "✓✗·⚠–";
    let folded: Vec<char> = folded(marks).chars().collect();

    assert_eq!(folded.len(), marks.chars().count(), "one for one");
    let mut seen = folded.clone();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen.len(), folded.len(), "and all different: {folded:?}");
}

#[test]
fn punctuation_folds_the_way_a_typewriter_would() {
    assert_eq!(folded("one — two"), "one -- two");
    assert_eq!(folded("→ do this"), "-> do this");
    assert_eq!(folded("wait…"), "wait...");
    assert_eq!(folded("“quoted”"), "\"quoted\"");
    assert_eq!(folded("‘quoted’"), "'quoted'");
    assert!(folded("plain ascii").is_ascii());
}

#[test]
fn text_that_needs_no_folding_is_unchanged() {
    assert_eq!(folded("nothing to do here"), "nothing to do here");
}

/// A terminal that can show it gets it as written; one that cannot gets it
/// folded. Asked of the decision rather than of the latch, so the test settles
/// nothing for the tests after it.
#[test]
fn what_a_terminal_gets_depends_on_what_it_can_show() {
    assert_eq!(shown("kept — as written", false), "kept — as written");
    assert_eq!(shown("folded — as needed", true), "folded -- as needed");
}

/// Settling twice is not two answers. A surface that asked second is told what
/// is in force, because that is what its output will actually be rendered as —
/// and being told otherwise is how a caller comes to believe a fold happened
/// that did not.
///
/// The locale here is deliberately the one the first call would have chosen
/// anyway: this test settles a process-wide latch, and the tests beside it read
/// the decision directly rather than the latch, so what is latched must be the
/// ordinary answer rather than one of them.
#[test]
fn what_is_settled_first_is_what_every_later_caller_is_told() {
    let first = settle(Some("en_GB.UTF-8"));

    assert!(!first, "a UTF-8 terminal is not folded");
    assert_eq!(
        settle(Some("C")),
        first,
        "the second caller is told what is in force, not what it asked for"
    );
}

/// Settled once, like the locale beside it, and for the same reason: it is a
/// property of the run rather than of any call.
///
/// The value latched here is deliberately the default one. This settles a
/// process-wide value, and every test beside it expects output for a person —
/// so what is latched has to be that, or this test would decide for them.
#[test]
fn who_the_output_is_for_is_settled_once_too() {
    let first = settle_audience(false);

    assert!(
        !first,
        "output is read by a person unless a run says otherwise"
    );
    assert_eq!(
        settle_audience(true),
        first,
        "the second caller is told what is in force, not what it asked for"
    );
    assert_eq!(for_a_parser(), first, "and asking plainly agrees");
}

/// The two ends of the funnel. Asserted only to the extent that they run: what
/// they put where is the architecture test's to guard, and reading back this
/// process's own streams would be a harness rather than a test.
#[test]
fn both_ends_of_the_funnel_take_a_line() {
    said("an ordinary line — folded or not");
    complained("a line about a failure");
    asked("and a question — answered beside it");
    emitted("{\"and\":\"a document nobody reads\"}");
    refused("{\"nor\":\"this one\"}");
}
