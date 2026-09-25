//! The walks and guards that keep going, and handing the terminal over.

use super::*;

/// A walk takes the key, a line to type on and an explicit yes before it reaches
/// a command — and the command is one of the core's own, carrying what was typed.
#[test]
fn a_walk_takes_a_key_a_word_and_an_answer_before_it_reaches_a_command() {
    let mut acting = starting("walkthrough");
    assert!(showing(&acting).contains("What to look for"));

    for character in "Sintel".chars() {
        acting.pressed(&Press::Typed(character));
    }
    assert_eq!(acting.pressed(&Press::Accept), Wanted::Nothing);
    let asking = showing(&acting);
    assert!(asking.contains("Walk through Sintel?"), "{asking}");

    assert_eq!(
        acting.pressed(&Press::Typed('y')),
        Wanted::Carry(Command::Walkthrough {
            item: Some("Sintel".to_owned())
        })
    );
}

/// Naming nothing is a request rather than a half-finished one, so it is put as
/// one — and what it reaches is the walk that suggests something.
#[test]
fn a_walk_asked_for_nothing_is_asked_about_as_its_own_request() {
    let mut acting = starting("walkthrough");

    acting.pressed(&Press::Accept);
    let asking = showing(&acting);

    assert!(asking.contains("Find something"), "{asking}");
    assert!(!asking.contains("Walk through?"), "{asking}");
    assert_eq!(
        acting.pressed(&Press::Typed('y')),
        Wanted::Carry(Command::Walkthrough { item: None })
    );
}

/// A walk's steps are put on the screen as they arrive, in the words a shell is
/// given for the same step — and the one that is kept for the report is not said
/// aloud here either.
#[test]
fn a_walk_says_each_step_in_the_words_a_shell_is_given_for_it() {
    let mut acting = starting("walkthrough");
    acting.pressed(&Press::Accept);
    acting.pressed(&Press::Typed('y'));

    acting.stepped(&Step::at(WalkStep::Choosing));
    assert!(showing(&acting).contains("asking the services"));

    acting.stepped(&Step::searched(3, 47));
    let said = showing(&acting);

    assert!(said.contains("Searching indexers"), "{said}");
    assert!(said.contains("47 releases"), "{said}");
    assert!(!said.contains("asking the services"), "{said}");
}

/// And what it came to goes under them rather than over them, which is the order
/// a shell shows them in.
#[test]
fn what_a_walk_came_to_goes_under_the_steps_that_were_watched() {
    let mut acting = starting("walkthrough");
    acting.pressed(&Press::Accept);
    acting.pressed(&Press::Typed('y'));
    acting.stepped(&Step::searched(3, 47));

    acting.came_to(Err(Box::new(a_failure())));
    let said = showing(&acting);

    assert!(said.contains("Searching indexers"), "{said}");
    assert!(said.contains("could not be reached"), "{said}");
}

/// A walk naming an action no surface offers reaches no command and says so.
/// Nothing on the list is one, and that is what the guard beside the list holds;
/// this is the arm that would carry a name that stopped being offered.
#[test]
fn a_walk_naming_an_action_nothing_offers_says_so() {
    let mut acting = Acting::opened();

    let wanted = lasting::beginning(
        &mut acting.stage,
        &lasting::tests::NOTHING_ANSWERS,
        lasting::Begun::Looked("Sintel".to_owned()),
        &Press::Typed('y'),
    );

    assert_eq!(wanted, Wanted::Nothing);
    let said = showing(&acting);
    assert!(said.contains("There is no action named"), "{said}");
}

/// A guard is chosen one of the stack's own forms, and the whole stack is not
/// among them: the translation refuses a guard with nothing to stop, so that
/// choice never reaches the operator. Nothing here decides that a second time.
#[test]
fn a_guard_is_offered_the_forms_and_never_the_whole_stack() {
    let mut acting = starting("watch");
    acting.told(Ok(Outcome::Forms(a_listing())));
    let offered = showing(&acting);

    assert!(offered.contains("Full stack"), "{offered}");
    assert!(offered.contains("Lean stack"), "{offered}");
    assert!(
        !offered.contains("the whole stack, rather than one form"),
        "{offered}"
    );
}

/// A stack declaring no forms leaves a guard nothing to be given, and the words
/// the operator is refused in are the web surface's own.
#[test]
fn a_guard_over_a_stack_that_declares_no_forms_is_refused_in_the_webs_words() {
    let mut acting = starting("watch");

    acting.told(Ok(Outcome::Forms(nothing_declared())));
    let said = showing(&acting);

    assert!(said.contains("forms"), "{said}");
}

/// The screen, with a guard running over one form.
fn guarding() -> Acting {
    let mut acting = starting("watch");
    acting.told(Ok(Outcome::Forms(a_listing())));
    acting.pressed(&Press::Accept);
    acting.pressed(&Press::Typed('y'));
    acting
}

/// A guard takes the same three steps and reaches the core's own command.
#[test]
fn a_guard_takes_a_key_a_form_and_an_answer_before_it_reaches_a_command() {
    let mut acting = starting("watch");
    acting.told(Ok(Outcome::Forms(a_listing())));
    assert_eq!(acting.pressed(&Press::Accept), Wanted::Nothing);
    let asking = showing(&acting);
    assert!(asking.contains("Guard the data location"), "{asking}");

    assert_eq!(
        acting.pressed(&Press::Typed('y')),
        Wanted::Carry(Command::Watch {
            forms: vec!["full".to_owned()]
        })
    );
}

/// The one thing on this screen with no ending of its own is the one the screen
/// offers to end, and an interruption is what ends it — which on a terminal in
/// raw mode is a keypress rather than a signal.
#[test]
fn an_interruption_lets_a_guard_go_and_says_what_it_left_alone() {
    let mut acting = guarding();

    assert_eq!(acting.pressed(&Press::Abandon), Wanted::Stop);

    let said = showing(&acting);
    assert!(said.contains("let go"), "{said}");
    assert!(said.contains("as they were"), "{said}");
    assert_eq!(
        acting.staying_for(),
        None,
        "a guard that has been let go is not something to wait for"
    );
}

/// And leaving does not end it. The stage is still the guard, so the run is
/// still holding the task — and what is said on the way out says it will not end
/// by itself and what does end it.
#[test]
fn leaving_a_guard_leaves_it_guarding_and_says_so() {
    let mut acting = guarding();

    assert_eq!(acting.pressed(&Press::Typed('q')), Wanted::Leave);

    let leaving = acting.staying_for().unwrap_or_default();
    assert!(leaving.contains("still running"), "{leaving}");
    assert!(leaving.contains("data location is lost"), "{leaving}");
    assert!(leaving.contains("Ctrl-C ends it"), "{leaving}");
}

/// A walk ends by itself, so the screen offers no end for it: an interruption
/// leaves the screen the way it does for every other running thing here, and the
/// run waits for the walk on the ordinary terminal.
#[test]
fn a_walk_is_left_running_rather_than_offered_an_end() {
    let mut acting = starting("walkthrough");
    acting.pressed(&Press::Accept);
    acting.pressed(&Press::Typed('y'));

    assert_eq!(acting.pressed(&Press::Abandon), Wanted::Leave);

    let leaving = acting.staying_for().unwrap_or_default();
    assert!(leaving.contains("waiting for a walk through"), "{leaving}");
    assert!(!leaving.contains("stack claimed"), "{leaving}");
}

/// What a guard is doing is said on the footer rather than drawn over the panels
/// it is guarding, and a walk's steps are drawn because nothing behind them is
/// showing them.
#[test]
fn a_guard_leaves_the_panels_showing_and_a_walk_does_not() {
    let guard = guarding();
    assert_eq!(guard.pane(20, 100).map(|pane| pane.title), None);
    let footing = footing(&guard);
    assert!(footing.contains("esc lets it go"), "{footing}");
    assert!(footing.contains("leaves it guarding"), "{footing}");

    let mut walk = starting("walkthrough");
    walk.pressed(&Press::Accept);
    walk.pressed(&Press::Typed('y'));
    assert!(walk.pane(20, 100).is_some());
}

/// A walk that has said more than the box holds is moved through rather than
/// cut, the way every other answer on this screen is.
#[test]
fn a_walk_that_outgrows_its_box_is_moved_through() {
    let mut acting = starting("walkthrough");
    acting.pressed(&Press::Accept);
    acting.pressed(&Press::Typed('y'));
    for _ in 0..12 {
        acting.stepped(&Step::searched(3, 47));
    }

    assert_eq!(acting.pressed(&Press::Forward), Wanted::Nothing);

    assert!(showing(&acting).contains("1 more line above"));
}

/// A step said while nothing is walking changes nothing, which is what a walk
/// left running and then let go of would say into.
#[test]
fn a_step_said_with_no_walk_open_changes_nothing() {
    let mut acting = Acting::opened();

    acting.stepped(&Step::searched(3, 47));

    assert_eq!(showing(&acting), String::new());
}

/// Each of the three lists is opened by its own key, and backing out of any of
/// them leaves the screen as it was.
#[test]
fn backing_out_of_the_list_that_keeps_going_leaves_the_screen_alone() {
    let mut acting = Acting::opened();
    acting.pressed(&Press::Typed(lasting::KEY));
    assert!(showing(&acting).contains("keeps going"));
    acting.pressed(&Press::Forward);
    acting.pressed(&Press::Back);
    assert!(showing(&acting).contains("> a walk through"));

    assert_eq!(acting.pressed(&Press::Abandon), Wanted::Nothing);

    assert_eq!(showing(&acting), String::new());
}

/// And so does backing out of what either of them is given before the question.
#[test]
fn backing_out_of_what_one_is_given_leaves_the_screen_alone() {
    let mut walking = starting("walkthrough");
    assert_eq!(walking.pressed(&Press::Abandon), Wanted::Nothing);
    assert_eq!(showing(&walking), String::new());

    let mut guarding = starting("watch");
    guarding.told(Ok(Outcome::Forms(a_listing())));
    assert_eq!(guarding.pressed(&Press::Abandon), Wanted::Nothing);
    assert_eq!(showing(&guarding), String::new());
}

/// A key that means nothing where it was pressed changes nothing there — on the
/// list, at the question, and while something is running. A screen that acted on
/// a stray letter would be a screen nobody could lean on.
#[test]
fn a_key_that_means_nothing_where_it_was_pressed_changes_nothing() {
    let mut listing = Acting::opened();
    listing.pressed(&Press::Typed(lasting::KEY));
    assert_eq!(listing.pressed(&Press::Typed('z')), Wanted::Nothing);
    assert!(showing(&listing).contains("keeps going"));

    let mut asking = starting("walkthrough");
    asking.pressed(&Press::Accept);
    assert_eq!(asking.pressed(&Press::Typed('n')), Wanted::Nothing);
    assert_eq!(showing(&asking), String::new());

    let mut running = guarding();
    assert_eq!(running.pressed(&Press::Typed('z')), Wanted::Nothing);
    let footing = footing(&running);
    assert!(footing.contains("esc lets it go"), "{footing}");
}

/// What a walk is told to look for is typed, taken back and moved over the way
/// every other line on this screen is.
#[test]
fn the_line_a_walk_is_typed_on_takes_back_what_was_typed() {
    let mut acting = starting("walkthrough");

    for character in "Sintelx".chars() {
        acting.pressed(&Press::Typed(character));
    }
    acting.pressed(&Press::Rubout);
    acting.pressed(&Press::Forward);
    acting.pressed(&Press::Back);
    acting.pressed(&Press::Accept);

    assert!(showing(&acting).contains("Walk through Sintel?"));
}

/// And the forms a guard could be given are moved over the way every other list
/// on this screen is.
#[test]
fn the_forms_a_guard_could_be_given_are_moved_over_like_any_list() {
    let mut acting = starting("watch");
    acting.told(Ok(Outcome::Forms(a_listing())));

    acting.pressed(&Press::Forward);
    acting.pressed(&Press::Typed('z'));
    acting.pressed(&Press::Back);
    acting.pressed(&Press::Accept);

    assert!(showing(&acting).contains("Guard the data location while running Full stack?"));
}

/// What a guard came to is the whole of its box, because it had said nothing
/// until then — and it is rendered by the renderer the command line reaches for
/// the same answer, whether it ended or failed.
#[test]
fn what_a_guard_came_to_is_the_whole_of_its_box() {
    let mut failed = guarding();
    failed.came_to(Err(Box::new(a_failure())));
    let said = showing(&failed);
    assert!(said.contains("could not be reached"), "{said}");
    assert!(said.contains("what it came to"), "{said}");

    let mut ended = guarding();
    ended.came_to(Ok(Outcome::Watch(SupervisionReport {
        forms: vec!["full".to_owned()],
        reason: "the data location is no longer present".to_owned(),
        stopped: true,
        would: None,
    })));
    let ending = showing(&ended);
    assert!(ending.contains("no longer present"), "{ending}");
}

/// The question before the terminal is handed over names what it costs, since
/// what it costs is the screen somebody was reading.
#[test]
fn the_key_that_hands_the_terminal_over_says_so_before_it_does() {
    let mut acting = Acting::opened();

    acting.pressed(&Press::Typed(surface::KEY));

    let said = showing(&acting);
    assert!(said.contains("web interface"), "{said}");
    assert!(said.contains("Close this screen"), "{said}");
    assert!(said.contains("any other key changes nothing"), "{said}");
}

/// The footer names every key the screen answers, including the two this slice
/// added, since a key nobody is told about is a key nobody presses.
#[test]
fn the_footer_names_the_keys_that_keep_going_and_the_one_that_hands_over() {
    let footing = footing(&Acting::opened());

    assert!(footing.contains(&format!("{} keeps going", lasting::KEY)));
    assert!(footing.contains(&format!("{} web", surface::KEY)));
}
