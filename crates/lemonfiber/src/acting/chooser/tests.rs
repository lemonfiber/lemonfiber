use super::Chooser;
use crate::acting::offer::Choice;
use lemonfiber_core::app::Command;

/// A choice by name, the command behind it being beside the point here.
fn a_choice(name: &str) -> Choice {
    Choice {
        name: name.to_owned(),
        about: format!("what {name} is for"),
        names: vec![name.to_owned()],
        marked: Some(false),
        command: Command::Up {
            forms: vec![name.to_owned()],
        },
    }
}

/// A chooser over three, for the movement tests.
fn three() -> Chooser<Choice> {
    Chooser::over(a_choice("one"), vec![a_choice("two"), a_choice("three")])
}

/// The names in the order they are drawn, and which one is marked.
fn shown(chooser: &Chooser<Choice>) -> Vec<(bool, String)> {
    chooser
        .listed()
        .map(|(here, choice)| (here, choice.name.clone()))
        .collect()
}

/// The name of the one selected, read off the list the screen is given rather
/// than off a field, so what is asserted is what an operator would see marked.
fn selected(chooser: &Chooser<Choice>) -> String {
    chooser
        .listed()
        .filter(|(here, _)| *here)
        .map(|(_, choice)| choice.name.clone())
        .collect()
}

#[test]
fn the_first_choice_is_the_one_selected() {
    let chooser = three();

    assert_eq!(selected(&chooser), "one");
    assert_eq!(
        shown(&chooser),
        vec![
            (true, "one".to_owned()),
            (false, "two".to_owned()),
            (false, "three".to_owned()),
        ]
    );
}

/// A chooser over one choice is still a chooser, which is what the two actions
/// that can mean the whole stack come to on a stack declaring no forms.
#[test]
fn one_choice_is_a_list_of_one() {
    let mut chooser = Chooser::over(a_choice("only"), Vec::new());
    chooser.forward();
    chooser.back();

    assert_eq!(selected(&chooser), "only");
    assert_eq!(shown(&chooser), vec![(true, "only".to_owned())]);
}

/// Moving down and back up again lands where it started, and the list is drawn
/// in the order it was offered throughout — a cursor that reordered what it
/// moved over would be a list nobody could read twice.
#[test]
fn moving_over_the_list_never_reorders_it() {
    let mut chooser = three();

    chooser.forward();
    chooser.forward();
    assert_eq!(selected(&chooser), "three");
    assert_eq!(
        shown(&chooser),
        vec![
            (false, "one".to_owned()),
            (false, "two".to_owned()),
            (true, "three".to_owned()),
        ]
    );

    chooser.back();
    chooser.back();
    assert_eq!(selected(&chooser), "one");
    assert_eq!(
        shown(&chooser),
        vec![
            (true, "one".to_owned()),
            (false, "two".to_owned()),
            (false, "three".to_owned()),
        ]
    );
}

/// The ends hold. A cursor that wrapped would put the operator on the teardown
/// after one press too many on a list they thought they were at the top of.
#[test]
fn the_ends_of_the_list_hold() {
    let mut chooser = three();

    chooser.back();
    assert_eq!(selected(&chooser), "one");

    for _ in 0..5 {
        chooser.forward();
    }
    assert_eq!(selected(&chooser), "three");
}

#[test]
fn what_is_taken_is_what_was_selected() {
    let mut chooser = three();
    chooser.forward();

    assert_eq!(chooser.taken().name, "two");
}

/// Every entry can be reached to change, in the order it was offered and with
/// the selected one told apart — which is what putting a mark on the row under
/// the cursor needs, and what taking the marks off the rest needs.
#[test]
fn every_entry_can_be_changed_where_it_was_offered() {
    let mut chooser = three();
    chooser.forward();

    for (here, choice) in chooser.each() {
        if here {
            choice.name = format!("{} (here)", choice.name);
        }
    }

    assert_eq!(
        shown(&chooser),
        vec![
            (false, "one".to_owned()),
            (true, "two (here)".to_owned()),
            (false, "three".to_owned()),
        ]
    );
}

/// Taking them all takes them in the order they were offered, which is the order
/// the question over several of them names them in. A list that came back in
/// cursor order would name them in an order nobody had seen.
#[test]
fn taking_them_all_takes_them_in_the_order_they_were_offered() {
    let mut chooser = three();
    chooser.forward();
    chooser.forward();

    let names: Vec<String> = chooser
        .all()
        .into_iter()
        .map(|choice| choice.name)
        .collect();

    assert_eq!(
        names,
        vec!["one".to_owned(), "two".to_owned(), "three".to_owned()]
    );
}
