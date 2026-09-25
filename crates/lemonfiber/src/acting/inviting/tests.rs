use super::{
    allowing, limited, named, over, unrated, ALLOW_UNRATED, ASKS_LIBRARIES, HELD_BACK,
    HOLD_UNRATED, LET_THROUGH, NO_LIMIT,
};
use crate::acting::errand::{self, Errand};
use crate::acting::{Press, Stage};
use lemonfiber_api::actions::Arguments;
use lemonfiber_core::age_limit;

/// The errand these two stages belong to, taken off the list every surface reads
/// it from rather than built here: a fixture of its own would let this file pass
/// while the screen offered something else.
fn inviting() -> &'static Errand {
    errand::tests::sending("invite").unwrap_or(errand::all().0)
}

/// One press over whichever half is open, leaving any other stage alone.
///
/// The two halves are driven through the same door a keypress arrives at rather
/// than by calling into either, because what is being tested is a sequence: the
/// name is typed, the line is gone by the time the list is shown, and the question
/// at the end has to say all three answers.
fn press(stage: &mut Stage, pressed: &Press) {
    match std::mem::replace(stage, Stage::Idle) {
        Stage::Allowing {
            errand,
            name,
            typed,
        } => {
            let _ = allowing(stage, errand, name, typed, pressed);
        }
        Stage::Limiting { errand, chooser } => {
            let _ = limited(stage, errand, chooser, pressed);
        }
        Stage::Unrated { errand, chooser } => {
            let _ = unrated(stage, errand, chooser, pressed);
        }
        other => *stage = other,
    }
}

/// Type a word, one character at a time.
fn typing(stage: &mut Stage, word: &str) {
    for character in word.chars() {
        press(stage, &Press::Typed(character));
    }
}

/// What the question at the end was given, and what it says — empty where the
/// errand reached no question, which fails whatever the assertion was.
fn agreed(stage: &Stage) -> (Arguments, String) {
    match stage {
        Stage::Agreeing { given, .. } | Stage::Weighing { given, .. } => {
            (given.asked(), given.said().to_owned())
        }
        _ => (Arguments::default(), String::new()),
    }
}

/// A stage that reached no question is read as having been given nothing.
///
/// The reader above says so, and every assertion below leans on it: a stage that
/// stopped early has to come back empty rather than carrying whatever the last
/// answered question left, or a test would pass on an errand that never asked.
#[test]
fn a_stage_that_asked_nothing_is_read_as_empty() {
    let (asked, said) = agreed(&Stage::Idle);

    assert_eq!(asked, Arguments::default());
    assert!(said.is_empty(), "{said}");
}

/// A line of library names becomes the names, with the spaces and the empty
/// pieces a stray comma leaves dropped.
#[test]
fn a_typed_line_becomes_the_libraries_it_named() {
    assert_eq!(
        named(" Films , Shows ,"),
        vec!["Films".to_owned(), "Shows".to_owned()]
    );
}

/// An empty line names no library, which is every one rather than none.
#[test]
fn an_empty_line_names_no_library() {
    assert!(named("").is_empty());
    assert!(named("  ,  ").is_empty());
}

/// The rows offered are no limit and the core's own steps, said in the core's own
/// words, so a step added there is on this list without anybody editing it.
#[test]
fn the_rows_are_no_limit_and_the_steps_the_core_offers() {
    assert!(
        age_limit::steps().len() > 1,
        "the core offers no ladder to read"
    );
    assert!(
        !NO_LIMIT.is_empty(),
        "the row that opens the list says nothing about what taking it comes to"
    );
    assert!(
        ASKS_LIBRARIES.contains("none is all of them"),
        "the line does not say what naming none comes to"
    );
}

/// The libraries typed and the age limit taken off the list reach the question
/// together, as one sentence saying all three answers.
///
/// What is under test is that the second answer does not lose the first: the line
/// the name was typed on is gone by the time the list is on the screen.
#[test]
fn what_was_typed_and_what_was_taken_reach_the_question_together() {
    let mut stage = over(inviting(), "ana".to_owned());

    typing(&mut stage, "Films, Shows");
    press(&mut stage, &Press::Accept);
    // The row under no limit, which is the lowest step the core offers.
    press(&mut stage, &Press::Forward);
    press(&mut stage, &Press::Accept);
    // Something was narrowed, so the third question is asked; the row it opens on
    // is the one a restriction defaults to everywhere else.
    press(&mut stage, &Press::Accept);

    let (asked, said) = agreed(&stage);
    assert_eq!(asked.name.as_deref(), Some("ana"), "{said}");
    assert_eq!(asked.libraries, ["Films".to_owned(), "Shows".to_owned()]);
    assert_eq!(
        asked.age_limit,
        age_limit::steps().first().map(|step| step.age),
        "the row under no limit did not become the youngest step the core offers"
    );
    assert!(
        said.contains("ana") && said.contains("Films, Shows"),
        "the question did not say what it was given: {said}"
    );
}

/// Pressing enter through the errand chooses the ordinary case — every library and
/// no age limit, which is the invitation this screen always sent.
#[test]
fn pressing_enter_through_it_chooses_every_library_and_no_limit() {
    let mut stage = over(inviting(), "ana".to_owned());

    press(&mut stage, &Press::Accept);
    press(&mut stage, &Press::Accept);

    let (asked, said) = agreed(&stage);
    assert!(asked.libraries.is_empty(), "{:?}", asked.libraries);
    assert_eq!(asked.age_limit, None, "{said}");
    assert!(
        said.contains("everything"),
        "the question did not say that naming none is all of them: {said}"
    );
}

/// Content the media server has no rating for is asked about last, and only where
/// the answers before it narrowed something.
///
/// An offer that names neither a library nor a limit writes no policy at all, so
/// the question would be about a setting the run does not touch — and a keypress
/// the ordinary case does not owe.
#[test]
fn what_happens_to_unrated_content_is_asked_only_where_something_was_narrowed() {
    let mut stage = over(inviting(), "ana".to_owned());
    press(&mut stage, &Press::Accept);
    press(&mut stage, &Press::Accept);
    assert!(
        matches!(&stage, Stage::Weighing { .. }),
        "an offer that narrowed nothing was asked about unrated content"
    );

    let mut stage = over(inviting(), "ana".to_owned());
    typing(&mut stage, "Films");
    press(&mut stage, &Press::Accept);
    press(&mut stage, &Press::Accept);
    assert!(
        matches!(&stage, Stage::Unrated { .. }),
        "an offer that narrowed a library was not asked about unrated content"
    );
}

/// The two answers reach the question, and the row it opens on is the one a
/// restriction defaults to on every other surface.
#[test]
fn the_answer_about_unrated_content_reaches_the_question_with_the_rest() {
    for (presses, expected) in [(0, HELD_BACK.to_owned()), (1, LET_THROUGH.to_owned())] {
        let mut stage = over(inviting(), "ana".to_owned());
        typing(&mut stage, "Films");
        press(&mut stage, &Press::Accept);
        press(&mut stage, &Press::Accept);
        for _ in 0..presses {
            press(&mut stage, &Press::Forward);
        }
        press(&mut stage, &Press::Accept);

        let (asked, said) = agreed(&stage);
        assert_eq!(asked.name.as_deref(), Some("ana"), "{said}");
        assert_eq!(asked.libraries, ["Films".to_owned()], "{said}");
        // Both rows send a word. The answer is a choice with a cost either way,
        // so the row an operator lands on is an answer they gave rather than one
        // this screen left for somewhere else to fill in.
        assert_eq!(asked.unrated.as_deref(), Some(expected.as_str()), "{said}");
    }
}

/// Both rows say what taking them comes to, because either answer has a cost.
#[test]
fn both_answers_about_unrated_content_say_what_they_cost() {
    assert!(HOLD_UNRATED.contains("invisible"), "{HOLD_UNRATED}");
    assert!(ALLOW_UNRATED.contains("unpredictable"), "{ALLOW_UNRATED}");
}

/// Backing out of the last question closes the errand rather than sending it with
/// an answer nobody gave.
#[test]
fn backing_out_of_the_last_question_closes_it() {
    let mut stage = over(inviting(), "ana".to_owned());
    typing(&mut stage, "Films");
    press(&mut stage, &Press::Accept);
    press(&mut stage, &Press::Accept);
    press(&mut stage, &Press::Abandon);

    assert!(matches!(stage, Stage::Idle), "the last list did not close");
}

/// The last list answers no keypress it has no use for.
#[test]
fn the_last_list_answers_no_press_it_has_no_use_for() {
    let mut stage = over(inviting(), "ana".to_owned());
    typing(&mut stage, "Films");
    press(&mut stage, &Press::Accept);
    press(&mut stage, &Press::Accept);
    press(&mut stage, &Press::Typed('x'));
    press(&mut stage, &Press::Rubout);
    press(&mut stage, &Press::Forward);
    press(&mut stage, &Press::Back);
    press(&mut stage, &Press::Accept);

    let (asked, said) = agreed(&stage);
    assert_eq!(
        asked.unrated.as_deref(),
        Some(HELD_BACK),
        "typing at the last list moved it: {said}"
    );
}

/// Taking a character back takes one character back, and moving down the list of
/// age limits and back up comes back to where it opened.
#[test]
fn a_line_is_corrected_and_a_list_moves_both_ways() {
    let mut stage = over(inviting(), "ana".to_owned());

    typing(&mut stage, "Filmsx");
    press(&mut stage, &Press::Rubout);
    assert!(
        matches!(&stage, Stage::Allowing { typed, .. } if typed == "Films"),
        "the line did not take a character back"
    );

    press(&mut stage, &Press::Accept);
    press(&mut stage, &Press::Forward);
    press(&mut stage, &Press::Back);
    press(&mut stage, &Press::Accept);
    // A library was named, so the third question stands between the list and the
    // question at the end.
    press(&mut stage, &Press::Accept);

    let (asked, said) = agreed(&stage);
    assert!(
        matches!(&stage, Stage::Weighing { .. }),
        "the errand did not go to the core with what it would grant: {said}"
    );
    assert_eq!(
        asked.age_limit, None,
        "moving down the list and back up did not come back to no limit: {said}"
    );
}

/// Backing out of either half closes the errand rather than going on with half an
/// answer.
#[test]
fn backing_out_of_either_half_closes_it() {
    let mut stage = over(inviting(), "ana".to_owned());
    typing(&mut stage, "Films");
    press(&mut stage, &Press::Abandon);
    assert!(matches!(stage, Stage::Idle), "the line did not close");

    let mut stage = over(inviting(), "ana".to_owned());
    press(&mut stage, &Press::Accept);
    press(&mut stage, &Press::Abandon);
    assert!(matches!(stage, Stage::Idle), "the list did not close");
}

/// Neither half answers a keypress it has no use for, and neither answers one that
/// arrives after the errand has moved on.
#[test]
fn neither_half_answers_a_press_it_has_no_use_for() {
    let mut stage = over(inviting(), "ana".to_owned());

    press(&mut stage, &Press::Forward);
    assert!(
        matches!(&stage, Stage::Allowing { typed, .. } if typed.is_empty()),
        "moving over a line changed it"
    );

    press(&mut stage, &Press::Accept);
    press(&mut stage, &Press::Typed('x'));
    press(&mut stage, &Press::Rubout);
    press(&mut stage, &Press::Accept);
    let (asked, said) = agreed(&stage);
    assert_eq!(
        asked.age_limit, None,
        "typing at the list of age limits moved it: {said}"
    );

    // What it would grant is with the core now, which belongs to the flow next
    // door, so a press arriving here is not either half's to answer.
    press(&mut stage, &Press::Accept);
    assert!(
        matches!(&stage, Stage::Weighing { .. }),
        "a press after the errand moved on was answered here"
    );
}
