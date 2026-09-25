//! Which action a name reaches, and how a name nothing offers is refused.

use super::acting::*;
use super::{command, naming, nothing, refusal};
use axum::http::StatusCode;
use lemonfiber_api::actions::{named, Arguments, Refused, OFFERED};
use lemonfiber_core::app::Command;

/// A removal with none named has lost the only part of it that decides what goes.
///
/// Required rather than defaulted, and the reason is the subject: a request that
/// lost a word in transit must not become a removal nobody asked for.
#[test]
fn a_removal_with_no_tier_named_says_which_argument_it_needs() {
    assert_eq!(
        named("uninstall", Arguments::default()).err(),
        Some(Refused::Missing {
            action: "uninstall".to_owned(),
            argument: "tier".to_owned(),
        })
    );
}

/// And a word naming none of the four is refused by name, with the four listed —
/// never read as whichever the shape would default to.
#[test]
fn a_removal_this_build_does_not_know_is_refused_with_the_four_that_exist() {
    let refused = named(
        "uninstall",
        Arguments {
            tier: Some("everything".to_owned()),
            ..Arguments::default()
        },
    )
    .err();

    assert!(
        matches!(
            refused,
            Some(Refused::Unrecognised { ref argument, ref offered })
                if argument == "everything" && offered.contains("media")
        ),
        "{refused:?}"
    );
}

#[test]
fn every_action_this_surface_offers_reaches_a_command() {
    // The whole guarantee, in one sweep: a name on the list that reached no
    // command would be an action only this surface has, and a command is what
    // the command line produces too.
    let unreachable: Vec<&str> = OFFERED
        .iter()
        .copied()
        // Named with exactly what the table says each takes: no less, since a
        // missing argument is refused, and no more, since one the command cannot
        // carry is refused too. So a refusal here is about the name.
        .filter(|action| named(action, exactly_what(action)).is_err())
        .collect();
    assert!(
        unreachable.is_empty(),
        "these are offered and reach nothing: {unreachable:?}"
    );
}

#[test]
fn a_name_this_surface_does_not_offer_is_refused_rather_than_invented() {
    assert_eq!(
        refusal("reticulate", nothing()),
        Some(Refused::Unknown {
            name: "reticulate".to_owned()
        })
    );
}

#[test]
fn a_read_is_not_an_action() {
    // Asking what the stack is doing is a read with an endpoint of its own, and
    // a write surface that also answered it would be two ways to ask one thing.
    // `logs` is here too, and it is the one worth stating: following runs for
    // minutes and is answered with a name like an action, but it is still the read
    // it always was, so it is asked for at the endpoint that already answers it.
    for read in [
        "status",
        "ps",
        "trace",
        "household",
        "stuck",
        "config-get",
        "logs",
    ] {
        assert!(
            matches!(refusal(read, nothing()), Some(Refused::Unknown { .. })),
            "{read} is a read"
        );
    }
}

#[test]
fn a_guard_reaches_the_command_and_the_forms_it_will_stop() {
    assert_eq!(
        command("watch", naming("tv")),
        Some(Command::Watch {
            forms: vec!["tv".to_owned()]
        })
    );
}

/// Both halves of hosting need to be told which command, and by a word that names one.
///
/// The two refusals are different and both matter: installing without saying which
/// would install whichever this program felt like, and a word naming none of them is
/// a mistake in the request rather than a request for something that does not exist.
/// The second names what it could have said, built from the list itself, so a command
/// that becomes hostable is offered here without anybody remembering to say so.
#[test]
fn hosting_is_told_which_command_or_refused_by_the_name_it_was_given() {
    for action in ["hosting-install", "hosting-remove"] {
        assert!(
            matches!(
                refusal(action, nothing()),
                Some(Refused::Missing { argument, .. }) if argument == "kept"
            ),
            "{action} was not told which command"
        );

        let named = Arguments {
            kept: Some("doctor".to_owned()),
            ..Arguments::default()
        };
        assert!(
            matches!(
                refusal(action, named),
                Some(Refused::Unrecognised { argument, offered })
                    if argument == "doctor" && offered.contains("watch") && offered.contains("expiring")
            ),
            "{action} took a word that names no long-running command"
        );
    }
}

#[test]
fn a_walk_reaches_the_command_and_the_one_thing_it_was_told_to_add() {
    let given = Arguments {
        item: Some("Sintel".to_owned()),
        ..Arguments::default()
    };
    assert_eq!(
        command("walkthrough", given),
        Some(Command::Walkthrough {
            item: Some("Sintel".to_owned())
        })
    );
}

#[test]
fn a_walk_told_nothing_in_particular_asks_to_be_suggested_something() {
    // Naming nothing is a request rather than an omission, and a field sent blank
    // asks the same thing as one left out — so a browser that renders an empty box
    // cannot turn a request into a walk for a title that is one space.
    for given in [
        nothing(),
        Arguments {
            item: Some("  ".to_owned()),
            ..Arguments::default()
        },
    ] {
        assert_eq!(
            command("walkthrough", given),
            Some(Command::Walkthrough { item: None })
        );
    }
}

#[test]
fn a_refusal_says_which_of_the_five_it_was() {
    let said = [
        Refused::Unknown {
            name: "reticulate".to_owned(),
        },
        Refused::Missing {
            action: "pull".to_owned(),
            argument: "forms".to_owned(),
        },
        Refused::Unrecognised {
            argument: "preset".to_owned(),
            offered: "try balanced".to_owned(),
        },
        Refused::Unwanted {
            action: "down".to_owned(),
            argument: "confirm".to_owned(),
        },
        Refused::Together {
            action: "down".to_owned(),
            argument: "wait".to_owned(),
            alongside: "services".to_owned(),
        },
    ];
    let spoken: BTreeSet<String> = said.iter().map(Refused::said).collect();
    for refusal in &said {
        assert!(!refusal.said().is_empty(), "{refusal:?}");
    }
    assert_eq!(
        spoken.len(),
        said.len(),
        "and each says something different"
    );
}

#[test]
fn a_name_this_surface_does_not_offer_is_absent_before_its_arguments_are_judged() {
    // A name nothing answers to is the first thing wrong with such a request, and
    // saying what its arguments should have been would be answering about an
    // action that does not exist.
    let given = Arguments {
        confirm: true,
        ..Arguments::default()
    };
    assert_eq!(
        refusal("reticulate", given),
        Some(Refused::Unknown {
            name: "reticulate".to_owned()
        })
    );
}

#[test]
fn a_name_that_is_not_an_action_is_absent_and_a_bad_argument_is_a_mistake() {
    // Two different statuses because they are two different faults: one is a
    // path nobody wrote, the other a request that has one.
    assert_eq!(
        Refused::Unknown {
            name: "reticulate".to_owned()
        }
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        Refused::Missing {
            action: "pull".to_owned(),
            argument: "forms".to_owned()
        }
        .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        Refused::Unwanted {
            action: "down".to_owned(),
            argument: "confirm".to_owned()
        }
        .status(),
        StatusCode::BAD_REQUEST
    );
}
