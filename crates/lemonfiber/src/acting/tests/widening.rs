//! A diagnosis read and widened, and a trace followed further.

use super::*;

/// The screen, having read a diagnosis and agreed to the checks that disturb.
fn widened() -> Acting {
    let mut acting = asking("how this stack is doing");
    acting.came_to(Ok(Outcome::Doctor(a_diagnosis())));
    acting.pressed(&Press::Typed('y'));
    acting
}

/// Every key this screen answers arrives as something it can act on, and a key
/// it has no use for arrives as nothing rather than as a character it never
/// typed. Ctrl-C is read as backing out: raw mode no longer turns it into a
/// signal, so an operator who reaches for it is asking to leave.
#[test]
fn the_keyboard_reaches_this_screen_as_the_presses_it_answers() {
    assert!(matches!(
        read(KeyCode::Char('q'), KeyModifiers::NONE),
        Some(Press::Typed('q'))
    ));
    assert!(matches!(
        read(KeyCode::Backspace, KeyModifiers::NONE),
        Some(Press::Rubout)
    ));
    assert!(matches!(
        read(KeyCode::Esc, KeyModifiers::NONE),
        Some(Press::Abandon)
    ));
    assert!(matches!(
        read(KeyCode::Enter, KeyModifiers::NONE),
        Some(Press::Accept)
    ));
    assert!(matches!(
        read(KeyCode::Up, KeyModifiers::NONE),
        Some(Press::Back)
    ));
    assert!(matches!(
        read(KeyCode::Down, KeyModifiers::NONE),
        Some(Press::Forward)
    ));
    assert!(matches!(
        read(KeyCode::Char('c'), KeyModifiers::CONTROL),
        Some(Press::Abandon)
    ));
    assert!(read(KeyCode::Home, KeyModifiers::NONE).is_none());
    // A character held with control is not that character: an operator typing
    // ctrl-d on this screen has not asked to stop the stack.
    assert!(read(KeyCode::Char('d'), KeyModifiers::CONTROL).is_none());
}

/// Every key an action is on begins that action, and every one of them reaches
/// the same list of what it can be given — under that action's own name, which
/// is what proves the answer landed on the offer the key began.
#[test]
fn every_key_an_action_is_on_begins_that_action() {
    for (key, named) in [
        ('u', "start"),
        ('d', "stop"),
        ('s', "switch"),
        ('t', "restart"),
        ('p', "fetch"),
    ] {
        let mut acting = Acting::opened();

        let wanted = acting.pressed(&Press::Typed(key));
        acting.told(Ok(Outcome::Forms(a_listing())));

        assert_eq!(wanted, Wanted::Ask(Command::Forms), "{key}");
        let said = showing(&acting);
        assert!(said.contains(named), "{key}: {said}");
    }
}

/// The whole flow of a read, which is the claim the other half of this screen
/// exists to make: one key, a question taken off a list, and a command that is
/// one of the core's own rather than one assembled here.
#[test]
fn a_question_takes_a_key_and_a_choice_before_it_reaches_a_command() {
    let mut acting = Acting::opened();

    assert_eq!(
        acting.pressed(&Press::Typed(question::KEY)),
        Wanted::Nothing
    );
    let said = showing(&acting);
    assert!(said.contains("> versions"), "{said}");
    assert!(said.contains("the container engine"), "{said}");

    assert_eq!(
        acting.pressed(&Press::Accept),
        Wanted::Carry(Command::Version)
    );
    assert!(showing(&acting).contains("waiting for this stack to answer"));
}

/// The claim this slice makes, end to end. The panels already show storage and
/// VPN facts a diagnosis reads, and a fact is not a verdict: what a check found,
/// and what to do about it, were on no screen at all. So the diagnosis is asked
/// for like any other read, its verdicts and its remedies are what the box shows,
/// and the checks that disturb a running system are offered *under* that report
/// rather than in front of it — the account is the run that named the gap.
#[test]
fn the_diagnosis_is_read_and_then_widened_under_its_own_report() {
    let report = a_diagnosis();
    let printed = crate::render::shaped(&Outcome::Doctor(report.clone())).text();
    let mut acting = at("how this stack is doing");

    assert_eq!(
        acting.pressed(&Press::Accept),
        Wanted::Carry(Command::Doctor {
            narrowing: Narrowing::Suite,
            disruptive: false,
            accept: None,
        })
    );

    acting.came_to(Ok(Outcome::Doctor(report)));
    let said = showing(&acting);
    for line in printed.lines().filter(|line| !line.is_empty()) {
        assert!(said.contains(line), "{line:?} is missing from {said}");
    }
    assert!(
        said.contains("Run the disruptive check"),
        "the remedy: {said}"
    );
    assert!(said.contains("including the ones that disturb?"), "{said}");
    assert!(
        said.contains("takes the tunnel away"),
        "what it costs: {said}"
    );

    assert_eq!(
        acting.pressed(&Press::Typed('y')),
        Wanted::Carry(Command::Doctor {
            narrowing: Narrowing::Suite,
            disruptive: true,
            accept: None,
        })
    );
}

/// A diagnosis narrowed to one family is widened over that family and no other.
/// Both disturbing checks name what to run in what they tell the operator, and a
/// screen that could only widen the whole suite would take the tunnel away to
/// spend one indexer search.
#[test]
fn a_narrowed_diagnosis_is_widened_over_the_narrowing_it_was_asked_with() {
    let mut acting = asking("one family of checks");
    for character in "vpn".chars() {
        acting.pressed(&Press::Typed(character));
    }

    assert_eq!(
        acting.pressed(&Press::Accept),
        Wanted::Carry(Command::Doctor {
            narrowing: Narrowing::Category(Category::Vpn),
            disruptive: false,
            accept: None,
        })
    );

    acting.came_to(Ok(Outcome::Doctor(a_diagnosis())));

    assert_eq!(
        acting.pressed(&Press::Typed('y')),
        Wanted::Carry(Command::Doctor {
            narrowing: Narrowing::Category(Category::Vpn),
            disruptive: true,
            accept: None,
        })
    );
}

/// Every answer that carries no offer is closed by the same key rather than
/// acted on. A `y` over a setting sending anything at all would be an action
/// arriving through a key that puts a box away everywhere else on this screen.
#[test]
fn a_yes_over_an_answer_that_carries_no_offer_puts_it_away() {
    let mut acting = asking("settings");
    acting.came_to(Ok(Outcome::Trace(a_trace("The Expanse"))));

    let said = showing(&acting);
    assert!(!said.contains("Ask the indexers"), "{said}");
    assert_eq!(acting.pressed(&Press::Typed('y')), Wanted::Nothing);

    assert!(showing(&acting).is_empty());
}

/// A trace does carry one, and it is the search rather than the checks.
///
/// The plain reading says in as many words that whether the indexers carry
/// nothing or the quality in force wants none of what they carry is not known, so
/// the offer that answers it sits under exactly the account that named the gap —
/// and the yes sends the search for the show that was followed, never a diagnosis.
#[test]
fn a_yes_over_a_trace_asks_the_indexers_about_the_show_it_followed() {
    let mut acting = asking_where();
    for character in "The Expanse".chars() {
        acting.pressed(&Press::Typed(character));
    }
    acting.pressed(&Press::Accept);
    acting.came_to(Ok(Outcome::Trace(a_trace("The Expanse"))));

    let said = showing(&acting);
    assert!(said.contains("Ask the indexers"), "{said}");
    assert!(said.contains("spends a live search"), "{said}");

    assert_eq!(
        acting.pressed(&Press::Typed('y')),
        Wanted::Carry(Command::Trace {
            term: "The Expanse".to_owned(),
            season: None,
            searching: true,
        })
    );
    // And the foot of the screen names the search rather than the checks, which
    // are not what is running.
    let footing = footing(&acting);
    assert!(footing.contains("search against the indexers"), "{footing}");
}

/// A diagnosis that could not be run offers nothing to widen. There is no report
/// to read, so there is no account for the question to sit under — and offering
/// to disturb the stack on the strength of a failure would be offering it about
/// nothing.
#[test]
fn a_diagnosis_that_could_not_be_run_offers_no_widening() {
    let mut acting = asking("how this stack is doing");

    acting.came_to(Err(Box::new(a_failure())));

    let said = showing(&acting);
    assert!(
        said.contains("the container engine could not be reached"),
        "{said}"
    );
    assert!(!said.contains("disturb"), "{said}");
    assert_eq!(acting.pressed(&Press::Typed('y')), Wanted::Nothing);
}

/// While the widened run is with the core the panels are the report — the VPN
/// panel is showing the very thing being tested — so nothing is drawn over them
/// and the footer says what is running. Leaving is the only thing left to ask,
/// and it leaves the run going: it took the tunnel away and has to put it back.
#[test]
fn a_run_that_disturbs_reports_through_the_panels_and_is_left_rather_than_stopped() {
    let mut acting = widened();

    assert!(showing(&acting).is_empty());
    let footing = footing(&acting);
    assert!(footing.contains("the checks that disturb"), "{footing}");
    assert_eq!(acting.pressed(&Press::Forward), Wanted::Nothing);

    assert_eq!(acting.pressed(&Press::Typed('q')), Wanted::Leave);
}

/// What it came to is read where an action's answer is read, and offers no
/// second widening: it is already the run that disturbs.
#[test]
fn what_the_widened_run_came_to_is_read_without_a_second_offer() {
    let report = a_diagnosis();
    let printed = crate::render::shaped(&Outcome::Doctor(report.clone())).text();
    let mut acting = widened();

    acting.came_to(Ok(Outcome::Doctor(report)));

    let said = showing(&acting);
    assert!(said.contains("what it came to"), "{said}");
    for line in printed.lines().filter(|line| !line.is_empty()) {
        assert!(said.contains(line), "{line:?} is missing from {said}");
    }
    assert!(!said.contains("including the ones that disturb?"), "{said}");
}
