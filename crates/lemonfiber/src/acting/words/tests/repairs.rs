//! The repairs and warnings offered, as the screen draws them.

use super::*;

/// One of the two things that can be done about a diagnosis, by its action.
fn a_mending(action: &str) -> &'static Mending {
    mending::tests::doing(action)
}

/// The list of what to do about a diagnosis is drawn the way every other list on
/// this screen is, so what an operator learned on one box carries to the next.
#[test]
fn what_to_do_about_a_diagnosis_is_listed_the_way_the_errands_are() {
    let (first, rest) = mending::all();

    let said = said(&Stage::Righting(Chooser::over(first, rest)), 20, 90);

    assert!(said.contains(" put right "), "{said}");
    assert!(said.contains("> what is wrong put right"), "{said}");
    assert!(
        said.contains("a warning you have already weighed"),
        "{said}"
    );
    assert!(said.contains("enter goes on"), "{said}");
}

/// While the offer is with the core the box says what it is waiting for, rather
/// than going quiet under a screen that is still gathering.
#[test]
fn an_offer_being_worked_out_says_what_it_is_waiting_for() {
    let said = said(&Stage::Looking(a_mending("repair")), 20, 90);

    assert!(said.contains("what is wrong put right"), "{said}");
    assert!(
        said.contains("working out what could be put right"),
        "{said}"
    );
}

/// The offer is a list that takes several, so each repair is a row that can be
/// marked on its own — which is the whole of what makes the yes a selection.
#[test]
fn the_repairs_offered_are_a_list_that_takes_several() {
    let said = said(&mending::tests::an_offer(), 20, 100);

    assert!(said.contains("what is wrong put right"), "{said}");
    assert!(said.contains("> [ ] vpn.port-forward-client"), "{said}");
    assert!(said.contains("[ ] config.wiring"), "{said}");
    assert!(said.contains("space marks"), "{said}");
}

/// The question sits under what the repairs marked would do, what else changes if
/// they do, and whether they can be put back. An effect somebody reads after
/// agreeing to it is not one they agreed to.
#[test]
fn the_question_over_a_repair_sits_under_what_it_would_do() {
    let said = said(&mending::tests::an_agreement(), 20, 100);

    assert!(said.contains("put vpn.port-forward-client back"), "{said}");
    assert!(said.contains("pauses briefly"), "{said}");
    assert!(said.contains("cannot be put back"), "{said}");
    assert!(
        said.contains("Put right vpn.port-forward-client?"),
        "{said}"
    );
    assert!(said.contains("y goes ahead"), "{said}");
}

/// The warnings are a list that takes one, because an accept answers one warning.
#[test]
fn the_warnings_are_a_list_that_takes_one() {
    let said = said(&mending::tests::warned_about(), 20, 100);

    assert!(
        said.contains("a warning you have already weighed"),
        "{said}"
    );
    assert!(said.contains("> vpn.unprotected"), "{said}");
    assert!(!said.contains("space marks"), "{said}");
}

/// The question over a warning names the check it answers and says what
/// answering it comes to, with no account above it — there is no run that would
/// report what accepting would do, and a preamble invented for symmetry would be
/// this screen claiming a rehearsal happened.
#[test]
fn the_question_over_a_warning_names_the_check_it_answers() {
    let said = said(&mending::tests::an_answer(), 20, 100);

    assert!(said.contains("Accept vpn.unprotected?"), "{said}");
    assert!(said.contains("stops leading"), "{said}");
    assert!(said.contains("y goes ahead"), "{said}");
    assert!(!said.contains("up and down move"), "{said}");
}

/// While it runs, nothing is drawn over the panels — a repair reaches the
/// services and the panel that says what each one is doing is behind this box —
/// and the footer says what is running and what leaving would leave.
#[test]
fn a_run_that_puts_things_right_covers_nothing_and_is_named_in_the_footer() {
    let running = Stage::Putting(a_mending("repair"));

    let said = said(&running, 20, 200);
    let footing = text(&footer(&running, 200));
    let staying = staying_for(&running);

    assert!(said.is_empty(), "{said}");
    assert!(footing.contains("what is wrong put right"), "{footing}");
    assert!(footing.contains("still running"), "{footing}");
    assert_eq!(
        staying,
        Some(
            "waiting for what is wrong put right to finish — leaving it now would leave \
             the stack claimed"
                .to_owned()
        )
    );
}

/// The question over several names how many rather than one of them, because
/// naming one of three would be naming the wrong thing about the other two.
#[test]
fn a_question_over_several_repairs_says_how_many_it_is_over() {
    let (_, first) = mending::tests::pressed(mending::tests::an_offer(), &Press::Typed(' '));
    let (_, moved_down) = mending::tests::pressed(first, &Press::Forward);
    let (_, both) = mending::tests::pressed(moved_down, &Press::Typed(' '));
    let (_, asked) = mending::tests::pressed(both, &Press::Accept);

    let said = said(&asked, 20, 100);

    assert!(said.contains("Put right the 2 above?"), "{said}");
}

/// A narrow screen shortens rather than running past its edge, whichever of the
/// boxes is open on it.
#[test]
fn no_row_runs_past_the_edge_of_a_narrow_screen() {
    let opened = [
        Stage::Choosing {
            offer: &A_START,
            chooser: two(),
        },
        Stage::Confirming {
            offer: &A_START,
            taken: a_taking("Full stack", "everything, behind the tunnel"),
        },
        Stage::Confirming {
            offer: &A_START,
            taken: several(),
        },
        Stage::Wondering(Chooser::over(&A_TRACE, Vec::new())),
        Stage::Typing {
            question: &A_TRACE,
            said: Vec::new(),
            typed: "something with a very long name indeed".to_owned(),
        },
        Stage::Waiting {
            question: &A_TRACE,
            said: vec!["The Expanse".to_owned()],
        },
        Stage::Answered {
            question: &A_TRACE,
            widening: None,
            reading: nine(),
        },
        Stage::Answered {
            question: &A_DIAGNOSIS,
            widening: a_widening(),
            reading: nine(),
        },
        Stage::Narrowing {
            question: &A_STUCK,
            chooser: two_listed(),
        },
        mending::tests::an_offer(),
        mending::tests::an_agreement(),
        mending::tests::warned_about(),
        mending::tests::an_answer(),
        Stage::Came(nine()),
    ];

    for stage in &opened {
        for across in [4usize, 12, 24, 40] {
            for row in said(stage, 20, across).lines().skip(1) {
                assert!(row.chars().count() <= across, "at {across}: {row:?}");
            }
        }
    }
}
