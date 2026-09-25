//! The lists and the questions over them, as the screen draws them.

use super::*;

/// While something is running the footer says so, and says what leaving does —
/// which is the one thing an operator watching a teardown needs to know. What it
/// must not say is that the work outlives the screen: the process drawing it is
/// the one carrying the work out.
#[test]
fn a_running_action_says_what_leaving_the_screen_does_about_it() {
    let stage = Stage::Running {
        offer: &A_START,
        taken: a_taking("Full stack", "everything"),
    };

    let said = text(&footer(&stage, 120));

    assert!(said.contains("Full stack"), "{said}");
    assert!(said.contains("still running"), "{said}");
    assert!(said.contains("closes the screen and waits"), "{said}");
    assert!(!said.contains("the work goes on"), "{said}");
}

/// Leaving mid-action says which action is being waited on and why, because an
/// operator who pressed q and got a wait instead of a shell is owed both.
#[test]
fn leaving_mid_action_says_what_is_being_waited_on_and_why() {
    let stage = Stage::Running {
        offer: &A_START,
        taken: a_taking("Full stack", "everything"),
    };

    let said = staying_for(&stage).unwrap_or_default();

    assert!(said.contains("start Full stack"), "{said}");
    assert!(said.contains("leave the stack claimed"), "{said}");
    assert!(
        staying_for(&Stage::Idle).is_none(),
        "nothing runs, nothing said"
    );
}

/// A form's own name reaches an ordinary terminal here rather than a drawn row,
/// and a control character is an instruction to both.
#[test]
fn a_name_from_somewhere_else_is_made_safe_before_it_is_said() {
    let stage = Stage::Running {
        offer: &A_START,
        taken: a_taking("Full\u{1b}[2Jstack", "everything"),
    };

    let said = staying_for(&stage).unwrap_or_default();

    assert!(!said.contains('\u{1b}'), "{said:?}");
    assert!(said.contains("Full[2Jstack"), "{said}");
}

/// A running action draws no box, because the panels behind it are the report.
#[test]
fn a_running_action_leaves_the_screen_behind_it_visible() {
    let stage = Stage::Running {
        offer: &A_START,
        taken: a_taking("Full stack", "everything"),
    };

    assert!(pane(&stage, 20, 80).is_none());
    assert!(pane(&Stage::Idle, 20, 80).is_none());
    assert!(text(&footer(&Stage::Idle, 120)).contains("r refresh"));
}

/// The list says what each choice is for, marks the one selected, shows every
/// row as one that could be marked, and says how to take it.
#[test]
fn the_list_marks_what_is_selected_and_says_how_to_take_it() {
    let stage = Stage::Choosing {
        offer: &A_START,
        chooser: two(),
    };

    let said = said(&stage, 20, 80);

    assert!(said.contains("> [ ] Full stack"), "{said}");
    assert!(said.contains("  [ ] Lean stack"), "{said}");
    assert!(said.contains("the download clients only"), "{said}");
    assert!(said.contains("space marks"), "{said}");
}

/// The line under the list says what enter would do, and changes when that
/// changes — which is the whole of how the one ambiguity on a list that takes
/// several is resolved. A rule an operator has to remember is a rule they will
/// get wrong once, on the teardown.
#[test]
fn the_line_under_a_list_says_which_thing_enter_would_take() {
    let mut chooser = two();
    let nothing = said(
        &Stage::Choosing {
            offer: &A_START,
            chooser: two(),
        },
        20,
        80,
    );

    for (_, choice) in chooser.each() {
        choice.marked = Some(true);
    }
    let both = said(
        &Stage::Choosing {
            offer: &A_START,
            chooser,
        },
        20,
        80,
    );

    assert!(nothing.contains("enter takes this one"), "{nothing}");
    assert!(!nothing.contains("marked"), "{nothing}");
    assert!(both.contains("enter takes the 2 marked"), "{both}");
    assert!(both.contains("> [x] Full stack"), "{both}");
    assert!(both.contains("  [x] Lean stack"), "{both}");
}

/// A list that takes one draws no box beside its rows and offers no key for
/// marking, because there is nothing on it that could be taken with another.
#[test]
fn a_list_that_takes_one_offers_nothing_to_mark() {
    let said = said(
        &Stage::Wondering(Chooser::over(&A_TRACE, Vec::new())),
        20,
        80,
    );

    assert!(said.contains("> where one thing is"), "{said}");
    assert!(!said.contains("[ ]"), "{said}");
    assert!(!said.contains("space marks"), "{said}");
    assert!(said.contains("enter goes on"), "{said}");
}

/// Several named together are named in the question, one row each. A box saying
/// only how many there were would be asking somebody to remember what they
/// marked a moment ago, which on a teardown is the wrong thing to be unsure of.
#[test]
fn the_question_over_several_names_every_one_of_them() {
    let stage = Stage::Confirming {
        offer: &A_START,
        taken: several(),
    };

    let said = said(&stage, 20, 80);

    assert!(said.contains("Start 2 forms?"), "{said}");
    assert!(said.contains("Full stack"), "{said}");
    assert!(said.contains("Lean stack"), "{said}");
    assert!(said.contains("the download clients only"), "{said}");
    assert!(said.contains("any other key changes nothing"), "{said}");
}

/// More forms than the box has room for are counted rather than dropped: a
/// question that displayed two of four would be asking for agreement to
/// something other than what it showed.
#[test]
fn a_question_over_more_forms_than_fit_counts_the_rest() {
    let stage = Stage::Confirming {
        offer: &A_START,
        taken: several(),
    };

    let said = said(&stage, 4, 80);

    assert!(said.contains("Start 2 forms?"), "{said}");
    assert!(
        said.contains("1 more form than this screen has room for"),
        "{said}"
    );
}

/// A list longer than the screen counts what it left out rather than showing
/// one of two as though there were one.
#[test]
fn a_list_too_long_for_the_screen_counts_what_it_left_out() {
    let stage = Stage::Choosing {
        offer: &A_START,
        chooser: two(),
    };

    let short = said(&stage, 3, 80);
    let whole = said(&stage, 20, 80);

    assert!(short.contains("1 more choice"), "{short}");
    assert!(!whole.contains("more choice"), "{whole}");
}

/// The question names the action and what it is about, and says which way
/// saying nothing falls.
#[test]
fn the_question_names_what_is_about_to_happen_and_to_what() {
    let stage = Stage::Confirming {
        offer: &A_START,
        taken: a_taking("Full stack", "everything, behind the tunnel"),
    };

    let said = said(&stage, 20, 80);

    assert!(said.contains("Start Full stack?"), "{said}");
    assert!(said.contains("everything, behind the tunnel"), "{said}");
    assert!(said.contains("any other key changes nothing"), "{said}");
}

#[test]
fn the_questions_are_listed_the_way_the_choices_are() {
    let stage = Stage::Wondering(Chooser::over(&A_TRACE, Vec::new()));

    let said = said(&stage, 20, 80);

    assert!(said.contains("> where one thing is"), "{said}");
    assert!(said.contains("follow one show or film"), "{said}");
    assert!(said.contains("enter goes on"), "{said}");
}

/// A question that takes a word says what it wants, shows what has been typed,
/// and says which key asks it.
#[test]
fn a_question_taking_a_word_says_what_it_wants_and_what_was_typed() {
    let stage = Stage::Typing {
        question: &A_TRACE,
        said: Vec::new(),
        typed: "The Exp".to_owned(),
    };

    let said = said(&stage, 20, 80);

    assert!(said.contains("What to follow"), "{said}");
    assert!(said.contains("> The Exp"), "{said}");
    assert!(said.contains("enter goes on"), "{said}");
}

/// A question with the core says so, under the name of the question asked, so
/// an answer that is slow to arrive does not read as a screen that stopped.
#[test]
fn a_question_with_the_core_says_what_it_is_waiting_for() {
    let waiting = Stage::Waiting {
        question: &A_TRACE,
        said: vec!["The Expanse".to_owned()],
    };

    let said = said(&waiting, 20, 80);

    assert!(said.contains("where one thing is"), "{said}");
    assert!(said.contains("waiting for this stack to answer"), "{said}");
}

/// An answer is drawn under the name of the question it answers, so a box that
/// is still open a minute later still says what it was asked.
#[test]
fn an_answer_is_drawn_under_the_question_it_answers() {
    let stage = Stage::Answered {
        question: &A_TRACE,
        widening: None,
        reading: nine(),
    };

    let said = said(&stage, 20, 80);

    assert!(said.contains("where one thing is"), "{said}");
    assert!(said.contains("line 0"), "{said}");
    assert!(said.contains("any other key closes"), "{said}");
    assert!(!said.contains("disturb"), "{said}");
}

/// A diagnosis is the one answer with a question under it: the report goes on
/// being moved through, and the line beneath it offers to run the same checks
/// again including the ones that disturb — which is what those very findings say
/// to do. An account read after agreeing is not one anybody agreed to.
#[test]
fn a_diagnosis_is_read_with_the_widening_offered_under_it() {
    let stage = Stage::Answered {
        question: &A_DIAGNOSIS,
        widening: a_widening(),
        reading: nine(),
    };

    let said = said(&stage, 20, 100);

    assert!(said.contains("how this stack is doing"), "{said}");
    assert!(said.contains("line 0"), "{said}");
    assert!(said.contains("including the ones that disturb?"), "{said}");
    assert!(said.contains("takes the tunnel away"), "{said}");
    assert!(said.contains("y goes ahead"), "{said}");
}

/// While it runs, nothing is drawn over the panels — the VPN panel behind this
/// box is the check being run — and the footer says what is running and what
/// leaving would leave the stack in.
///
/// Named off the run itself rather than off one written down here, because there
/// are two of these: a screen naming the checks that disturb while the indexers
/// are being searched would be telling an operator about the wrong wait.
#[test]
fn a_run_that_disturbs_covers_nothing_and_is_named_in_the_footer() {
    let running = Stage::Disturbing(&DIAGNOSIS);
    let said = said(&running, 20, 200);
    let footing = text(&footer(&running, 200));
    let staying = staying_for(&running);

    assert!(said.is_empty(), "{said}");
    assert!(footing.contains(DIAGNOSIS.name), "{footing}");
    assert!(footing.contains("still running"), "{footing}");
    assert_eq!(
        staying,
        Some(format!(
            "waiting for {} to finish — leaving it now would leave the stack claimed",
            DIAGNOSIS.name
        ))
    );
}

/// A listing offered to be taken one of is drawn under the name of the question
/// that listed it, and read as a list rather than as an answer — so what is on
/// the screen says both what was asked and that there is something to do with it.
#[test]
fn a_listing_to_take_one_of_is_drawn_under_the_question_that_listed_it() {
    let stage = Stage::Narrowing {
        question: &A_STUCK,
        chooser: two_listed(),
    };

    let said = said(&stage, 20, 80);

    assert!(said.contains("what is stuck"), "{said}");
    assert!(said.contains("> Full stack"), "{said}");
    assert!(said.contains("enter"), "{said}");
}

/// A control character in another service's account of a failure is an
/// instruction to a terminal, and the screen must not carry one.
#[test]
fn text_from_somewhere_else_is_made_safe_before_it_is_drawn() {
    let stage = Stage::Confirming {
        offer: &A_START,
        taken: a_taking("Full\u{1b}[2Jstack", "quietly clearing the screen"),
    };

    let said = said(&stage, 20, 120);

    assert!(!said.contains('\u{1b}'), "{said:?}");
    assert!(said.contains("Full[2Jstack"), "{said}");
}
