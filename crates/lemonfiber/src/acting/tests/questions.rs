//! Questions: asked, typed at, picked from, and answered.

use super::*;

/// Two stuck items, the second of them the one a test takes.
fn two_stuck() -> StuckReport {
    StuckReport {
        items: vec![a_stuck("The Expanse", "sonarr"), a_stuck("Dune", "radarr")],
        incomplete: false,
        unsupported: Vec::new(),
    }
}

/// One stuck item, held where it is said to be held.
fn a_stuck(title: &str, service: &str) -> StuckEntry {
    StuckEntry {
        title: title.to_owned(),
        service: service.to_owned(),
        stage: TraceStage::Downloading,
    }
}

/// An answer is shown in the words the command line gives for the same request,
/// rather than in a second account of it — which is the whole reason a question
/// names a read instead of this screen writing its own report.
#[test]
fn an_answer_is_shown_in_the_words_the_command_line_gives() {
    let outcome = Outcome::Version(a_version());
    let printed = crate::render::shaped(&outcome).text();
    let mut acting = Acting::opened();
    acting.pressed(&Press::Typed(question::KEY));
    acting.pressed(&Press::Accept);

    acting.came_to(Ok(outcome));

    let said = showing(&acting);
    assert!(said.contains("versions"), "the box is named for it: {said}");
    for line in printed.lines().filter(|line| !line.is_empty()) {
        assert!(said.contains(line), "{line:?} is missing from {said}");
    }
}

/// A read that could not be carried out says why, in the words the command line
/// gives for the same failure.
#[test]
fn a_question_the_stack_will_not_answer_says_why() {
    let mut acting = Acting::opened();
    acting.pressed(&Press::Typed(question::KEY));
    acting.pressed(&Press::Accept);

    acting.came_to(Err(Box::new(a_failure())));

    let said = showing(&acting);
    assert!(
        said.contains("the container engine could not be reached"),
        "{said}"
    );
}

/// A question that has to be given a word gets a line to type it on, takes
/// characters back one at a time, and only then reaches a command.
#[test]
fn a_question_that_takes_a_word_is_typed_before_it_is_asked() {
    let mut acting = asking_where();

    for character in "Expansee".chars() {
        acting.pressed(&Press::Typed(character));
    }
    acting.pressed(&Press::Rubout);
    let said = showing(&acting);
    assert!(said.contains("What to follow"), "{said}");
    assert!(said.contains("> Expanse"), "{said}");

    assert_eq!(
        acting.pressed(&Press::Accept),
        Wanted::Carry(Command::Trace {
            term: "Expanse".to_owned(),
            season: None,
            searching: false,
        })
    );
}

/// Asking it with nothing typed says what is missing, in the sentence the web
/// surface gives the same request — the refusal is that surface's, not one this
/// screen wrote.
#[test]
fn a_question_asked_with_no_word_says_what_is_missing() {
    let mut acting = asking_where();

    assert_eq!(acting.pressed(&Press::Accept), Wanted::Nothing);

    let said = showing(&acting);
    assert!(
        said.contains(lemonfiber_api::read::table::NO_TERM),
        "{said}"
    );
}

/// Moving over the line being typed changes neither it nor the screen, and
/// backing out of it leaves the screen clear.
#[test]
fn the_line_being_typed_ignores_a_move_and_is_left_on_a_way_out() {
    let mut acting = asking_where();
    acting.pressed(&Press::Typed('x'));

    assert_eq!(acting.pressed(&Press::Forward), Wanted::Nothing);
    assert!(showing(&acting).contains("> x"));

    assert_eq!(acting.pressed(&Press::Abandon), Wanted::Nothing);
    assert!(showing(&acting).is_empty());
}

/// A question narrowed by picking asks its own read for the whole listing, and
/// what comes back is offered as a list to take one of rather than as an answer.
///
/// Carried rather than asked for the reason every other question is: reading what
/// is stuck reaches the \*arrs over the network, and a screen that waited on it
/// would stop answering keys while it did.
#[test]
fn a_question_that_picks_asks_for_its_listing_and_then_offers_it() {
    let mut acting = at("what is stuck");

    assert_eq!(
        acting.pressed(&Press::Accept),
        Wanted::Carry(Command::Stuck)
    );

    acting.came_to(Ok(Outcome::Stuck(two_stuck())));
    let said = showing(&acting);
    assert!(said.contains("what is stuck"), "{said}");
    assert!(said.contains("> The Expanse"), "{said}");
    assert!(said.contains("Dune"), "{said}");
    assert!(said.contains("radarr, stuck at downloading"), "{said}");
}

/// Taking one of them follows that one, by the title the entry carries — which
/// is the second read, and the arrangement the web's own stuck list already has.
#[test]
fn taking_one_of_the_stuck_items_follows_that_one() {
    let mut acting = asking("what is stuck");
    acting.came_to(Ok(Outcome::Stuck(two_stuck())));

    acting.pressed(&Press::Forward);
    assert_eq!(
        acting.pressed(&Press::Accept),
        Wanted::Carry(Command::Trace {
            term: "Dune".to_owned(),
            season: None,
            searching: false,
        })
    );

    // And what *that* comes to is read as an answer, under the question that led
    // here. It is a trace rather than another stuck listing, which is the whole
    // of what the second read being a different read means: sent back through the
    // listing it would be a shape this question cannot list, and would have been
    // called unexpected on the one answer the operator actually asked for.
    acting.came_to(Ok(Outcome::Trace(a_trace("Dune"))));
    let said = showing(&acting);
    assert!(said.contains("what is stuck"), "{said}");
    assert!(said.contains("Dune"), "{said}");
}

/// Backing out of the second read leaves the screen clear, the way backing out of
/// the first does — and an answer nobody is waiting for then changes nothing.
#[test]
fn one_of_the_listed_things_can_be_left_while_it_is_with_the_core() {
    let mut acting = asking("what is stuck");
    acting.came_to(Ok(Outcome::Stuck(two_stuck())));
    acting.pressed(&Press::Accept);

    assert!(showing(&acting).contains("waiting for this stack to answer"));
    assert_eq!(acting.pressed(&Press::Typed('x')), Wanted::Nothing);
    assert!(showing(&acting).contains("waiting for this stack to answer"));

    assert_eq!(acting.pressed(&Press::Abandon), Wanted::Nothing);
    acting.came_to(Ok(Outcome::Trace(a_trace("Dune"))));
    assert!(showing(&acting).is_empty());
}

/// A listing with nothing in it is an answer, not a refusal — and it is the
/// answer the command line gives for the same read, rather than a sentence this
/// screen wrote about there being nothing to choose.
#[test]
fn a_listing_with_nothing_in_it_is_read_as_the_answer_it_is() {
    let mut acting = asking("what is stuck");
    acting.came_to(Ok(Outcome::Stuck(StuckReport::default())));

    let said = showing(&acting);
    assert!(said.contains("Nothing is stuck"), "{said}");
    assert!(!said.contains("enter"), "{said}");
}

/// An entry carrying no title cannot be followed, so it is not offered — and a
/// listing of nothing but those is the answer rather than an empty list.
#[test]
fn a_stuck_entry_with_nothing_to_follow_by_is_not_offered() {
    let mut acting = asking("what is stuck");
    acting.came_to(Ok(Outcome::Stuck(StuckReport {
        items: vec![a_stuck("", "sonarr"), a_stuck("Dune", "radarr")],
        incomplete: false,
        unsupported: Vec::new(),
    })));

    assert!(showing(&acting).contains("> Dune"));
    assert_eq!(
        acting.pressed(&Press::Accept),
        Wanted::Carry(Command::Trace {
            term: "Dune".to_owned(),
            season: None,
            searching: false,
        })
    );

    let mut acting = asking("what is stuck");
    acting.came_to(Ok(Outcome::Stuck(StuckReport {
        items: vec![a_stuck("", "sonarr")],
        incomplete: false,
        unsupported: Vec::new(),
    })));
    assert!(showing(&acting).contains("item(s) stuck"));
}

/// A failure on the second read is said under the question that led to it, the
/// way a failure on the first is — the operator asked one question and is owed
/// one account of what became of it.
#[test]
fn a_failure_following_one_of_them_is_said_under_the_question() {
    let mut acting = asking("what is stuck");
    acting.came_to(Ok(Outcome::Stuck(two_stuck())));
    acting.pressed(&Press::Accept);

    acting.came_to(Err(Box::new(a_failure())));

    let said = showing(&acting);
    assert!(said.contains("what is stuck"), "{said}");
    assert!(
        said.contains("the container engine could not be reached"),
        "{said}"
    );
}

/// The same shape over the forms: the listing is asked for, offered in the
/// stack's own words, and taking one says what starting it would come to.
#[test]
fn taking_one_of_the_forms_says_what_starting_it_would_come_to() {
    let mut acting = at("what starting one would come to");

    assert_eq!(
        acting.pressed(&Press::Accept),
        Wanted::Carry(Command::Forms)
    );

    acting.came_to(Ok(Outcome::Forms(a_listing())));
    let said = showing(&acting);
    assert!(said.contains("> Full stack"), "{said}");
    assert!(said.contains("the download clients only"), "{said}");

    assert_eq!(
        acting.pressed(&Press::Accept),
        Wanted::Carry(Command::Preview {
            forms: vec!["full".to_owned()],
        })
    );
}

/// A stack that declares no forms answers with the listing that says so.
#[test]
fn a_stack_declaring_no_forms_answers_the_listing_rather_than_a_list() {
    let mut acting = asking("what starting one would come to");
    acting.came_to(Ok(Outcome::Forms(nothing_declared())));

    assert!(!showing(&acting).is_empty());
}

/// Moving over a listing and leaving it take nothing, the way every other list
/// on this screen does.
#[test]
fn moving_over_a_listing_and_leaving_it_take_nothing() {
    let mut acting = asking("what is stuck");
    acting.came_to(Ok(Outcome::Stuck(two_stuck())));

    acting.pressed(&Press::Forward);
    acting.pressed(&Press::Back);
    assert_eq!(acting.pressed(&Press::Typed('y')), Wanted::Nothing);
    assert_eq!(acting.pressed(&Press::Rubout), Wanted::Nothing);
    assert!(showing(&acting).contains("> The Expanse"));

    assert_eq!(acting.pressed(&Press::Abandon), Wanted::Nothing);
    assert!(showing(&acting).is_empty());
}

/// An answer of a shape the question does not list is said to be unexpected
/// rather than drawn as an empty list nobody can take anything off.
#[test]
fn an_answer_a_picking_question_cannot_list_is_unexpected() {
    let mut acting = asking("what is stuck");
    acting.came_to(Ok(Outcome::Version(a_version())));

    assert!(!showing(&acting).is_empty());
    assert!(!showing(&acting).contains("0.8.0"));
}

/// The two narrowings that are typed are typed on the line a trace already had,
/// fill the argument their own read names, and are refused empty in that read's
/// own words.
#[test]
fn a_setting_and_a_member_are_named_on_the_line_a_trace_is_named_on() {
    let mut acting = asking("one setting");
    assert!(showing(&acting).contains("Which setting"));
    for character in "SONARR_API_KEY".chars() {
        acting.pressed(&Press::Typed(character));
    }
    assert_eq!(
        acting.pressed(&Press::Accept),
        Wanted::Carry(Command::ConfigGet {
            key: "SONARR_API_KEY".to_owned(),
        })
    );

    let mut acting = asking("what one person asked for");
    assert!(showing(&acting).contains("Which member"));
    for character in "ada".chars() {
        acting.pressed(&Press::Typed(character));
    }
    assert_eq!(
        acting.pressed(&Press::Accept),
        Wanted::Carry(Command::Household {
            member: Some("ada".to_owned()),
        })
    );
}

/// Naming neither is refused in the sentence a browser is refused with, which is
/// a different sentence for each of them because each names a different thing.
#[test]
fn naming_no_setting_and_naming_no_member_are_each_refused_in_their_own_words() {
    let mut acting = asking("one setting");
    assert_eq!(acting.pressed(&Press::Accept), Wanted::Nothing);
    let said = showing(&acting);
    assert!(
        said.contains(lemonfiber_api::read::table::NO_SETTING),
        "{said}"
    );

    let mut acting = asking("what one person asked for");
    assert_eq!(acting.pressed(&Press::Accept), Wanted::Nothing);
    let said = showing(&acting);
    assert!(
        said.contains(lemonfiber_api::read::table::NO_MEMBER),
        "{said}"
    );
}

/// Moving over the questions and typing at them take nothing, the way the list
/// of what an action can be given does.
#[test]
fn moving_over_the_questions_and_typing_at_them_take_nothing() {
    let mut acting = Acting::opened();
    acting.pressed(&Press::Typed(question::KEY));

    acting.pressed(&Press::Forward);
    acting.pressed(&Press::Back);
    assert_eq!(acting.pressed(&Press::Typed('y')), Wanted::Nothing);
    assert_eq!(acting.pressed(&Press::Rubout), Wanted::Nothing);
    assert!(showing(&acting).contains("> versions"));

    assert_eq!(acting.pressed(&Press::Abandon), Wanted::Nothing);
    assert!(showing(&acting).is_empty());
}

/// A question with the core waits for it, and can be left the way the listing
/// of an action's subjects can — after which its answer changes nothing, so a
/// reply nobody is waiting for cannot open a box over what came next.
#[test]
fn a_question_left_before_it_lands_takes_the_answer_nowhere() {
    let mut acting = Acting::opened();
    acting.pressed(&Press::Typed(question::KEY));
    acting.pressed(&Press::Accept);

    assert_eq!(acting.pressed(&Press::Forward), Wanted::Nothing);
    assert!(showing(&acting).contains("waiting for this stack to answer"));

    assert_eq!(acting.pressed(&Press::Abandon), Wanted::Nothing);
    acting.came_to(Ok(Outcome::Version(a_version())));

    assert!(showing(&acting).is_empty());
}

/// An answer longer than the box moves through it, and any key that is not a
/// move puts it away — the same dismissal the pane of words has.
#[test]
fn an_answer_moves_under_the_arrows_and_closes_under_anything_else() {
    let mut acting = Acting::opened();
    acting.pressed(&Press::Typed(question::KEY));
    acting.pressed(&Press::Accept);
    acting.came_to(Ok(Outcome::Version(a_version())));
    let opened = showing(&acting);

    assert_eq!(acting.pressed(&Press::Forward), Wanted::Nothing);
    let moved = showing(&acting);
    assert!(moved.contains("1 more line above"), "{moved}");
    assert_ne!(moved, opened);

    acting.pressed(&Press::Back);
    assert_eq!(showing(&acting), opened);

    assert_eq!(acting.pressed(&Press::Typed('n')), Wanted::Nothing);
    assert!(showing(&acting).is_empty());
}

/// What an action came to moves the same way, so two boxes an operator meets a
/// keypress apart do not answer the arrows differently.
#[test]
fn what_an_action_came_to_moves_the_same_way() {
    let (mut acting, _) = choosing('t', a_listing());
    acting.pressed(&Press::Accept);
    acting.pressed(&Press::Typed('y'));
    acting.came_to(Err(Box::new(a_failure())));
    let opened = showing(&acting);

    acting.pressed(&Press::Forward);

    assert_ne!(showing(&acting), opened);
    assert!(showing(&acting).contains("1 more line above"));
}
