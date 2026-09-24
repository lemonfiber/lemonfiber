//! Choosing an action: the key, the forms marked, and the question asked.

use super::*;

/// The whole flow, which is the claim this screen exists to make: a key, a
/// choice, a question, an explicit yes, and only then a command — and the
/// command is one of the core's own.
#[test]
fn an_action_takes_a_key_a_choice_and_an_answer_before_it_reaches_a_command() {
    let (mut acting, first) = choosing('t', a_listing());
    assert_eq!(first, Wanted::Ask(Command::Forms));

    assert_eq!(acting.pressed(&Press::Accept), Wanted::Nothing);
    let carried = acting.pressed(&Press::Typed('y'));

    assert_eq!(
        carried,
        Wanted::Carry(Command::Restart {
            forms: vec!["full".to_owned()],
            services: Vec::new(),
        })
    );
}

/// Moving back up the list lands on what it started at, and a stray character
/// pressed over a list is not an answer to it — a screen where any key took the
/// selected entry would act on a keypress meant for something else.
#[test]
fn moving_over_the_list_and_typing_at_it_take_nothing() {
    let (mut acting, _) = choosing('t', a_listing());

    acting.pressed(&Press::Forward);
    acting.pressed(&Press::Back);
    assert_eq!(acting.pressed(&Press::Typed('y')), Wanted::Nothing);
    let said = showing(&acting);
    assert!(said.contains("> [ ] Full stack"), "{said}");

    acting.pressed(&Press::Accept);
    let carried = acting.pressed(&Press::Typed('y'));

    assert_eq!(
        carried,
        Wanted::Carry(Command::Restart {
            forms: vec!["full".to_owned()],
            services: Vec::new(),
        })
    );
}

/// What a lifecycle command came to is shown in the words the command line
/// gives for the same run, rather than in a second account of it.
#[test]
fn a_lifecycle_report_is_shown_in_the_words_the_command_line_gives() {
    let report = a_lifecycle("restart", a_plan("full", Vec::new()));
    let printed = crate::render::stack::lifecycle(&report).text();
    let (mut acting, _) = choosing('t', a_listing());
    acting.pressed(&Press::Accept);
    acting.pressed(&Press::Typed('y'));

    acting.came_to(Ok(Outcome::Lifecycle(report)));

    let said = showing(&acting);
    let lines: Vec<&str> = printed.lines().filter(|line| !line.is_empty()).collect();
    // The words to compare against are what the command line rendered, and a
    // render that came back empty would make the comparison below hold over
    // nothing while the claim is that the two accounts are the same one.
    assert!(
        !lines.is_empty(),
        "the command line rendered nothing to compare"
    );
    for line in lines {
        assert!(said.contains(line), "{line:?} is missing from {said}");
    }
}

/// The claim this slice exists to make: the list names several forms at once,
/// which is what the command line takes and what a browser sends whole, and the
/// several reach one command over all of them rather than one command each.
#[test]
fn marking_several_forms_acts_on_every_one_of_them() {
    let (mut acting, _) = choosing('t', a_listing());

    acting.pressed(&Press::Typed(' '));
    acting.pressed(&Press::Forward);
    acting.pressed(&Press::Typed(' '));
    let listed = showing(&acting);
    assert_eq!(acting.pressed(&Press::Accept), Wanted::Nothing);
    let asked = showing(&acting);
    let carried = acting.pressed(&Press::Typed('y'));

    assert!(listed.contains("[x] Full stack"), "{listed}");
    assert!(listed.contains("[x] Lean stack"), "{listed}");
    assert!(listed.contains("enter takes the 2 marked"), "{listed}");
    assert!(asked.contains("Restart 2 forms?"), "{asked}");
    assert!(asked.contains("Full stack"), "{asked}");
    assert!(asked.contains("Lean stack"), "{asked}");
    assert_eq!(
        carried,
        Wanted::Carry(Command::Restart {
            forms: vec!["full".to_owned(), "lean".to_owned()],
            services: Vec::new(),
        })
    );
}

/// A mark comes off the way it went on, and a list with every mark taken off is
/// the list it was: enter falls back to the row under the cursor. Nobody should
/// have to abandon a list to escape a keypress they did not mean.
#[test]
fn taking_every_mark_off_again_leaves_the_row_under_the_cursor() {
    let (mut acting, _) = choosing('t', a_listing());

    acting.pressed(&Press::Typed(' '));
    acting.pressed(&Press::Typed(' '));
    let listed = showing(&acting);
    acting.pressed(&Press::Accept);
    let carried = acting.pressed(&Press::Typed('y'));

    assert!(listed.contains("enter takes this one"), "{listed}");
    assert_eq!(
        carried,
        Wanted::Carry(Command::Restart {
            forms: vec!["full".to_owned()],
            services: Vec::new(),
        })
    );
}

/// The whole stack is instead of naming forms rather than one more of them, so
/// marking it takes the marks off the forms and what is sent is the empty list
/// the command reads as everything — never that list with a form's name also in
/// it, which would be a request for something other than what was shown.
#[test]
fn marking_the_whole_stack_takes_the_marks_off_the_forms() {
    let (mut acting, _) = choosing('u', a_listing());

    acting.pressed(&Press::Forward);
    acting.pressed(&Press::Typed(' '));
    acting.pressed(&Press::Back);
    acting.pressed(&Press::Typed(' '));
    let listed = showing(&acting);
    acting.pressed(&Press::Accept);
    let carried = acting.pressed(&Press::Typed('y'));

    assert!(listed.contains("[x] everything"), "{listed}");
    assert!(listed.contains("[ ] Full stack"), "{listed}");
    assert_eq!(carried, Wanted::Carry(Command::Up { forms: Vec::new() }));
}

/// A guard is chosen off the same list by the same movement, so it names several
/// forms at once too. One list behaving two ways depending on which key opened it
/// is the thing the shared movement exists to prevent.
#[test]
fn a_guard_can_be_set_over_several_forms_at_once() {
    let mut acting = starting("watch");
    acting.told(Ok(Outcome::Forms(a_listing())));

    acting.pressed(&Press::Typed(' '));
    acting.pressed(&Press::Forward);
    acting.pressed(&Press::Typed(' '));
    acting.pressed(&Press::Accept);
    let asked = showing(&acting);
    let carried = acting.pressed(&Press::Typed('y'));

    assert!(asked.contains("2 forms?"), "{asked}");
    assert_eq!(
        carried,
        Wanted::Carry(Command::Watch {
            forms: vec!["full".to_owned(), "lean".to_owned()],
        })
    );
}

/// While several are running the footer says how many rather than running their
/// names together, and the line on the way out says the same — one row, on a
/// screen whose width belongs to the panels behind it.
#[test]
fn a_running_action_over_several_says_how_many_it_is_running_on() {
    let (mut acting, _) = choosing('t', a_listing());
    acting.pressed(&Press::Typed(' '));
    acting.pressed(&Press::Forward);
    acting.pressed(&Press::Typed(' '));
    acting.pressed(&Press::Accept);
    acting.pressed(&Press::Typed('y'));

    let footer = footing(&acting);
    let leaving = acting.staying_for().unwrap_or_default();

    assert!(footer.contains("restart 2 forms"), "{footer}");
    assert!(leaving.contains("restart 2 forms"), "{leaving}");
}

/// Choosing something further down the list acts on that one, which is the
/// difference between a list and a decoration.
#[test]
fn what_was_selected_is_what_is_acted_on() {
    let (mut acting, _) = choosing('t', a_listing());

    acting.pressed(&Press::Forward);
    acting.pressed(&Press::Accept);
    let carried = acting.pressed(&Press::Typed('Y'));

    assert_eq!(
        carried,
        Wanted::Carry(Command::Restart {
            forms: vec!["lean".to_owned()],
            services: Vec::new(),
        })
    );
}

/// The two actions whose command can carry an empty list offer the whole stack,
/// and taking it names no form at all.
#[test]
fn the_whole_stack_is_a_choice_where_the_command_can_mean_it() {
    let (mut acting, _) = choosing('d', a_listing());

    acting.pressed(&Press::Accept);
    let carried = acting.pressed(&Press::Typed('y'));

    assert_eq!(
        carried,
        Wanted::Carry(Command::Down {
            forms: Vec::new(),
            wait: Waiting::Never
        })
    );
}

/// Anything but an explicit yes changes nothing, and leaves the screen where an
/// operator can start again.
#[test]
fn a_question_answered_with_anything_else_changes_nothing() {
    for answer in [
        Press::Typed('n'),
        Press::Typed('\n'),
        Press::Accept,
        Press::Abandon,
        Press::Forward,
    ] {
        let (mut acting, _) = choosing('d', a_listing());
        acting.pressed(&Press::Accept);

        let answered = acting.pressed(&answer);

        assert_eq!(answered, Wanted::Nothing);
        assert!(showing(&acting).is_empty(), "the screen is clear again");
    }
}

/// The question names what is about to happen and what it is about to happen
/// to, before it happens rather than after.
#[test]
fn the_question_says_what_is_about_to_happen() {
    let (mut acting, _) = choosing('d', a_listing());
    acting.pressed(&Press::Forward);
    acting.pressed(&Press::Accept);

    let said = showing(&acting);

    assert!(said.contains("Stop Full stack?"), "{said}");
    assert!(said.contains("everything, behind the tunnel"), "{said}");
}

/// The three actions whose command refuses an empty list are refused where the
/// stack declares nothing to name, in the words the web surface uses.
#[test]
fn an_action_needing_a_form_is_refused_where_the_stack_declares_none() {
    for key in ['s', 't', 'p'] {
        let (acting, _) = choosing(key, nothing_declared());

        let said = showing(&acting);

        assert!(said.contains("needs `forms`"), "{key}: {said}");
    }
}

/// Backing out of the list leaves the stack alone and the screen clear.
#[test]
fn backing_out_of_the_list_leaves_the_screen_clear() {
    let (mut acting, _) = choosing('p', a_listing());
    assert!(!showing(&acting).is_empty());

    assert_eq!(acting.pressed(&Press::Abandon), Wanted::Nothing);

    assert!(showing(&acting).is_empty());
    assert_eq!(acting.pressed(&Press::Typed('q')), Wanted::Leave);
}

/// An answer is taken once, by the ask that is outstanding. A second one for the
/// same question changes nothing, so a reply that arrives twice cannot open a
/// list over whatever the operator has moved on to.
#[test]
fn an_answer_is_taken_once_by_the_ask_that_is_outstanding() {
    let (mut acting, _) = choosing('p', a_listing());
    assert!(showing(&acting).contains("Full stack"));
    acting.pressed(&Press::Abandon);

    acting.told(Ok(Outcome::Forms(a_listing())));

    assert!(showing(&acting).is_empty(), "the ask was already answered");
}

/// The keys the screen already answered go on answering, and a key on nothing
/// is not an action.
#[test]
fn the_keys_the_screen_already_answered_still_answer() {
    let mut acting = Acting::opened();

    assert_eq!(acting.pressed(&Press::Typed('r')), Wanted::Gather);
    assert_eq!(acting.pressed(&Press::Typed('q')), Wanted::Leave);
    assert_eq!(acting.pressed(&Press::Abandon), Wanted::Leave);
    assert_eq!(acting.pressed(&Press::Typed('z')), Wanted::Nothing);
    assert_eq!(acting.pressed(&Press::Back), Wanted::Nothing);
    assert_eq!(acting.pressed(&Press::Accept), Wanted::Nothing);
}

/// The words open on a key, close on any key, and the key that closed them does
/// nothing else — so putting them away can never start something.
#[test]
fn the_words_open_on_a_key_and_the_key_that_closes_them_does_nothing_else() {
    let mut acting = Acting::opened();

    assert_eq!(acting.pressed(&Press::Typed('?')), Wanted::Words);
    assert!(acting.showing_words());

    assert_eq!(acting.pressed(&Press::Typed('d')), Wanted::Nothing);
    assert!(!acting.showing_words());
    assert!(showing(&acting).is_empty(), "no action was begun");
}

/// A run that explains nothing has no pane of words to open, and the key that
/// would have opened it is not an action either.
#[test]
fn a_run_that_explains_nothing_opens_no_words() {
    let mut acting = Acting::opened().without_explanations();

    assert_eq!(acting.pressed(&Press::Typed('?')), Wanted::Nothing);
    assert!(!acting.showing_words());
}

/// While an action is running, the footer says so and the screen behind it is
/// left alone — the panels are the report, and covering them would take away
/// the one thing worth watching.
#[test]
fn a_running_action_says_so_on_the_footer_and_covers_nothing() {
    let (mut acting, _) = choosing('t', a_listing());
    acting.pressed(&Press::Accept);
    acting.pressed(&Press::Typed('y'));

    assert!(
        showing(&acting).is_empty(),
        "nothing is drawn over the panels"
    );
    let footer = footing(&acting);
    assert!(footer.contains("restart Full stack"), "{footer}");
    assert!(footer.contains("still running"), "{footer}");
}

/// Leaving while an action runs is allowed, and the action is still where it
/// was: what is being waited on is asked for on the way out, because the run
/// holding the screen is the run carrying the work out. Anything else pressed
/// leaves it running and says nothing.
#[test]
fn leaving_while_an_action_runs_says_what_is_still_being_waited_on() {
    let (mut acting, _) = choosing('t', a_listing());
    acting.pressed(&Press::Accept);
    acting.pressed(&Press::Typed('y'));

    assert_eq!(acting.pressed(&Press::Forward), Wanted::Nothing);
    assert!(footing(&acting).contains("still running"));

    assert_eq!(acting.pressed(&Press::Typed('q')), Wanted::Leave);
    let said = acting.staying_for().unwrap_or_default();
    assert!(said.contains("restart Full stack"), "{said}");
    assert!(said.contains("leave the stack claimed"), "{said}");
}

/// Leaving with nothing running has nothing to wait for, so an operator who
/// pressed q on an idle screen gets their shell rather than a sentence.
#[test]
fn leaving_with_nothing_running_waits_for_nothing() {
    let mut acting = Acting::opened();

    assert_eq!(acting.pressed(&Press::Typed('q')), Wanted::Leave);

    assert!(acting.staying_for().is_none());
}

/// What an action came to is put on the screen in the words the command line
/// gives for the same run, and put away by any key.
#[test]
fn what_an_action_came_to_is_shown_and_then_put_away() {
    let (mut acting, _) = choosing('t', a_listing());
    acting.pressed(&Press::Accept);
    acting.pressed(&Press::Typed('y'));

    acting.came_to(Err(Box::new(a_failure())));

    let said = showing(&acting);
    assert!(said.contains("what it came to"), "{said}");
    assert!(
        said.contains("the container engine could not be reached"),
        "{said}"
    );
    assert!(said.contains("Start the container engine"), "{said}");

    assert_eq!(acting.pressed(&Press::Typed('n')), Wanted::Nothing);
    assert!(showing(&acting).is_empty());
    assert!(footing(&acting).contains("r refresh"));
}

/// A stack that will not say what it declares is reported rather than left as a
/// key that seems to do nothing.
#[test]
fn a_stack_that_will_not_say_what_it_declares_says_why() {
    let mut acting = Acting::opened();
    acting.pressed(&Press::Typed('u'));

    acting.told(Err(Box::new(a_failure())));

    let said = showing(&acting);
    assert!(
        said.contains("the container engine could not be reached"),
        "{said}"
    );
}

/// An answer of the wrong shape is said rather than shown as nothing, because a
/// screen that went quiet reads as an action that never ran.
#[test]
fn an_answer_of_the_wrong_shape_is_said_rather_than_swallowed() {
    let mut asked = Acting::opened();
    asked.pressed(&Press::Typed('u'));
    asked.told(Ok(Outcome::Version(a_version())));

    let (mut acted, _) = choosing('t', a_listing());
    acted.pressed(&Press::Accept);
    acted.pressed(&Press::Typed('y'));
    acted.came_to(Ok(Outcome::Version(a_version())));

    let (asked, acted) = (showing(&asked), showing(&acted));
    assert!(asked.contains("something other than"), "{asked}");
    assert!(acted.contains("something other than"), "{acted}");
}

/// An answer arriving for a question nobody asked changes nothing, so a stale
/// reply cannot open a list over whatever the operator has moved on to.
#[test]
fn an_answer_nobody_is_waiting_for_changes_nothing() {
    let mut acting = Acting::opened();

    acting.told(Ok(Outcome::Forms(a_listing())));

    assert!(showing(&acting).is_empty());
}
