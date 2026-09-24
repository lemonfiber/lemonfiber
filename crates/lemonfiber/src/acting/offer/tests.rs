use super::{
    for_key, marking, or_refused, over, subjects, taking, Choice, Chooser, Command, Fills, Offer,
    Over, Press, Stage, Taken, OFFERED, WHOLE,
};
use lemonfiber_api::actions::{named, Arguments, OFFERED as WEB};
use lemonfiber_core::model::{FormReport, FormsReport};

/// A stack declaring two forms, as a listing shows them.
pub(crate) fn a_listing() -> FormsReport {
    FormsReport {
        forms: vec![
            FormReport {
                id: "full".to_owned(),
                name: "Full stack".to_owned(),
                description: "everything, behind the tunnel".to_owned(),
                composable: false,
            },
            FormReport {
                id: "lean".to_owned(),
                name: "Lean stack".to_owned(),
                description: "the download clients only".to_owned(),
                composable: true,
            },
        ],
    }
}

/// A stack that declares no forms at all.
pub(crate) fn nothing_declared() -> FormsReport {
    FormsReport { forms: Vec::new() }
}

/// The action one name is on, for the tests next door that want a particular one.
pub(crate) fn offering(action: &str) -> Option<&'static Offer> {
    OFFERED.iter().find(|offer| offer.action == action)
}

/// What one action's list of forms comes to with a form taken off it, which is
/// what the list of services beside it is opened having already been given.
///
/// Built through the translation the screen builds it through rather than by
/// hand, so the command it carries is the one that action reaches — and the row
/// naming no service, which goes on with exactly this, is held to something true.
pub(crate) fn a_form_taken(action: &str) -> Option<Taken> {
    let (first, rest) = super::guarding(action, &a_listing()).ok()?;
    over_forms(action, at(Chooser::over(first, rest), "Full stack")).ok()
}

/// One form, as a row of the list holds it.
fn a_form(id: &str) -> Choice {
    Choice {
        name: format!("{id} stack"),
        about: format!("what {id} is for"),
        names: vec![id.to_owned()],
        marked: Some(false),
        command: Command::Pull {
            forms: vec![id.to_owned()],
        },
    }
}

/// The whole stack, which is the one row of the list that names no form.
fn the_whole_stack() -> Choice {
    Choice {
        name: WHOLE.to_owned(),
        about: super::EVERY_FORM.to_owned(),
        names: Vec::new(),
        marked: Some(false),
        command: Command::Up { forms: Vec::new() },
    }
}

/// The list an action that can mean everything is given: the whole stack, then
/// two forms.
fn a_list() -> Chooser<Choice> {
    Chooser::over(the_whole_stack(), vec![a_form("full"), a_form("lean")])
}

/// The list taken, filling the forms, which is what every one of these lists is
/// for on this side of the screen.
fn over_forms(action: &str, chooser: Chooser<Choice>) -> Result<Taken, String> {
    taking(action, Fills::Forms, &Arguments::default(), chooser)
}

/// A press over such a list, which fills the forms for the same reason.
fn pressed(action: &str, chooser: Chooser<Choice>, press: &Press) -> Over {
    over(action, Fills::Forms, &Arguments::default(), chooser, press)
}

/// Which rows are marked, read off the list the screen is given rather than off
/// a field, so what is asserted is what an operator would see marked.
fn marked(chooser: &Chooser<Choice>) -> Vec<String> {
    chooser
        .listed()
        .filter(|(_, choice)| choice.marked == Some(true))
        .map(|(_, choice)| choice.name.clone())
        .collect()
}

/// The list with the cursor moved to the row of this name.
///
/// Back to the top first and then down, because the cursor may be below the row
/// being looked for and a walk that only goes one way would stop on whatever it
/// ended on — which is a mark put on the wrong row, silently.
fn at(mut chooser: Chooser<Choice>, name: &str) -> Chooser<Choice> {
    for _ in 0..3 {
        chooser.back();
    }
    for _ in 0..3 {
        if chooser
            .listed()
            .any(|(here, choice)| here && choice.name == name)
        {
            break;
        }
        chooser.forward();
    }
    chooser
}

/// Space marks the row under the cursor, and space again takes the mark off.
/// Both from the same key, because a mark nobody can undo is a mark somebody has
/// to abandon the whole list to escape.
#[test]
fn the_row_under_the_cursor_is_marked_and_unmarked_by_the_same_key() {
    let mut chooser = at(a_list(), "full stack");

    marking(&mut chooser);
    assert_eq!(marked(&chooser), vec!["full stack".to_owned()]);

    marking(&mut chooser);
    assert!(marked(&chooser).is_empty(), "{:?}", marked(&chooser));
}

/// The whole stack is *instead of* naming forms rather than one more of them.
/// What the two together would send is what the whole stack alone would send, so
/// a list showing both marked would be naming something it was not about to send.
#[test]
fn the_whole_stack_and_a_form_are_never_marked_together() {
    let mut chooser = at(a_list(), "full stack");
    marking(&mut chooser);
    let mut chooser = at(chooser, WHOLE);

    marking(&mut chooser);
    assert_eq!(marked(&chooser), vec![WHOLE.to_owned()]);

    let mut chooser = at(chooser, "lean stack");
    marking(&mut chooser);
    assert_eq!(marked(&chooser), vec!["lean stack".to_owned()]);
}

/// Several marked forms go to the translation as one list, and come back as the
/// one command the command line produces for the same request — rather than as
/// several commands this screen would have to decide the order of.
#[test]
fn several_marked_forms_reach_one_command_over_all_of_them() {
    let mut chooser = at(a_list(), "full stack");
    marking(&mut chooser);
    let mut chooser = at(chooser, "lean stack");
    marking(&mut chooser);

    let taken = over_forms("pull", chooser).ok();

    assert_eq!(
        taken.as_ref().map(|taken| taken.command.clone()),
        named(
            "pull",
            Arguments {
                forms: vec!["full".to_owned(), "lean".to_owned()],
                ..Arguments::default()
            }
        )
        .ok()
    );
    assert_eq!(taken.map(|taken| taken.name()), Some("2 forms".to_owned()));
}

/// Nothing marked is the list this screen has always had: enter takes the row
/// under the cursor. An operator who never presses the new key gets the screen
/// they had before it existed.
#[test]
fn nothing_marked_takes_the_row_under_the_cursor() {
    let taken = over_forms("pull", at(a_list(), "lean stack")).ok();

    assert_eq!(
        taken.as_ref().map(|taken| taken.command.clone()),
        Some(Command::Pull {
            forms: vec!["lean".to_owned()]
        })
    );
    assert_eq!(
        taken.map(|taken| taken.name()),
        Some("lean stack".to_owned())
    );
}

/// What the screen would say about a stage, as one piece of text.
fn said(stage: &Stage) -> String {
    crate::acting::words::pane(stage, 20, 100).map_or_else(String::new, |pane| {
        pane.lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<Vec<&str>>()
            .join(" ")
    })
}

/// A list the translation will not carry is refused in the words the web surface
/// gives, and those words are put where the operator is looking rather than
/// swallowed on the way to a command.
///
/// Nothing this screen offers can reach it: every action here and the guard next
/// door are names the web's table knows, which the test at the top of this list
/// and its twin beside the other one both hold. So it is driven through a name
/// that is not — and it is one path for both lists, because a refusal that read
/// differently under one key than under another would be this screen having an
/// opinion about a translation it does not own.
#[test]
fn a_list_the_translation_refuses_is_put_in_front_of_the_operator() {
    let mut chooser = at(a_list(), "full stack");
    marking(&mut chooser);
    let mut stage = Stage::Idle;

    let taken = or_refused(&mut stage, over_forms("nonsense", chooser));

    assert!(taken.is_none());
    assert!(said(&stage).contains("nonsense"), "{}", said(&stage));
}

/// The list a press left behind, where it left one.
fn still_choosing(pressed: Over) -> Option<Chooser<Choice>> {
    match pressed {
        Over::Choosing(chooser) => Some(chooser),
        Over::Taken(_) | Over::Left => None,
    }
}

/// Space reaches the marking, enter reaches the taking, and escape leaves the
/// list with nothing taken — the three things a press over this list can be.
#[test]
fn a_press_over_the_list_marks_takes_or_leaves_it() {
    let marked = still_choosing(pressed(
        "pull",
        at(a_list(), "full stack"),
        &Press::Typed(' '),
    ))
    .map(|chooser| marked(&chooser))
    .unwrap_or_default();
    assert_eq!(marked, vec!["full stack".to_owned()]);

    let taken = matches!(
        pressed("pull", at(a_list(), "full stack"), &Press::Accept),
        Over::Taken(Ok(_))
    );
    assert!(taken);

    assert!(still_choosing(pressed("pull", a_list(), &Press::Abandon)).is_none());
}

/// What one action can be given over this listing.
fn choices_for(action: &str, report: &FormsReport) -> Vec<Choice> {
    OFFERED
        .iter()
        .filter(|offer| offer.action == action)
        .filter_map(|offer| offer.given(report).ok())
        .flat_map(|(first, rest)| std::iter::once(first).chain(rest))
        .collect()
}

/// Why one action can be given nothing over this listing.
fn refusal_for(action: &str, report: &FormsReport) -> String {
    OFFERED
        .iter()
        .filter(|offer| offer.action == action)
        .filter_map(|offer| offer.given(report).err())
        .collect::<Vec<String>>()
        .join(" ")
}

/// The whole point of naming the action rather than assembling a command here:
/// what this screen offers has to be something another surface already offers,
/// or the requirement it is being built for is defeated by the thing built for
/// it.
#[test]
fn every_action_this_screen_offers_is_one_the_other_surfaces_offer() {
    let missing: Vec<&str> = OFFERED
        .iter()
        .map(|offer| offer.action)
        .filter(|action| !WEB.contains(action))
        .collect();

    assert!(missing.is_empty(), "{missing:?}");
}

/// One key per action, or the second is unreachable and nobody would know.
#[test]
fn no_two_actions_answer_to_the_same_key() {
    for offer in OFFERED {
        let same = OFFERED
            .iter()
            .filter(|other| other.key == offer.key)
            .count();
        assert_eq!(same, 1, "more than one action is on {:?}", offer.key);
    }
}

/// The four keys the screen already answers stay answered by it.
#[test]
fn no_action_takes_a_key_the_screen_already_uses() {
    for taken in [
        'q',
        'r',
        '?',
        crate::acting::question::KEY,
        crate::acting::errand::KEY,
    ] {
        assert!(for_key(taken).is_none(), "{taken:?} was already spoken for");
    }
}

#[test]
fn a_key_no_action_is_on_reaches_none() {
    assert!(for_key('z').is_none());
    assert!(for_key('u').is_some());
}

/// Naming nothing means the whole stack for the two whose command can carry an
/// empty list, and this screen learns which two by asking rather than by
/// keeping a second list that could come to disagree with the first.
#[test]
fn the_whole_stack_is_offered_only_where_the_command_can_carry_it() {
    let listing = a_listing();

    let offering_it: Vec<&str> = OFFERED
        .iter()
        .filter(|offer| {
            choices_for(offer.action, &listing)
                .iter()
                .any(|choice| choice.name == WHOLE)
        })
        .map(|offer| offer.action)
        .collect();

    assert_eq!(offering_it, vec!["up", "down"]);
}

/// Every form the stack declares is a choice, named as the stack names it.
#[test]
fn each_form_the_stack_declares_is_offered_in_the_stacks_own_words() {
    let choices = choices_for("restart", &a_listing());

    let named: Vec<&str> = choices.iter().map(|choice| choice.name.as_str()).collect();
    assert_eq!(named, vec!["Full stack", "Lean stack"]);
    let about: Vec<&str> = choices.iter().map(|choice| choice.about.as_str()).collect();
    assert!(about.contains(&"the download clients only"), "{about:?}");
}

/// The command a choice comes to is the command the other surfaces produce for
/// the same request, rather than one this screen assembled.
#[test]
fn a_choice_comes_to_the_command_every_surface_produces() {
    let asked = Arguments {
        forms: vec!["full".to_owned()],
        ..Arguments::default()
    };
    let wanted = named("restart", asked).ok();

    let reached = choices_for("restart", &a_listing())
        .into_iter()
        .next()
        .map(|choice| choice.command);

    assert!(wanted.is_some(), "the web surface translates this one");
    assert_eq!(reached, wanted);
}

/// A stack with no forms leaves an action that insists on one with nothing to
/// offer, and the words the operator gets are the words the web surface gives
/// for the same request rather than a sentence this screen wrote.
#[test]
fn an_action_that_insists_on_a_form_says_so_where_the_stack_declares_none() {
    let said = refusal_for("switch", &nothing_declared());

    assert!(said.contains("switch"), "{said}");
    assert!(said.contains("forms"), "{said}");
    assert!(choices_for("switch", &nothing_declared()).is_empty());
}

/// The two that can mean the whole stack still have something to offer there.
#[test]
fn an_action_that_can_mean_everything_offers_it_even_with_no_forms_declared() {
    let choices = choices_for("down", &nothing_declared());

    assert_eq!(choices.len(), 1);
    assert!(choices.iter().any(|choice| choice.name == WHOLE));
    assert!(refusal_for("down", &nothing_declared()).is_empty());
}

/// The whole stack leads, because it is the one choice that is not a form and
/// reading it after the forms would read as another of them.
#[test]
fn the_whole_stack_is_the_first_subject_offered() {
    let subjects = subjects(&a_listing());

    let first = subjects
        .first()
        .map(|(forms, name, _)| (forms.len(), name.clone()));
    assert_eq!(first, Some((0, WHOLE.to_owned())));
    assert_eq!(subjects.len(), 3);
}
