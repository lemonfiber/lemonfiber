use super::removal;
use lemonfiber_core::model::{HouseholdRemoval, Revoked};

/// A removal as it stands before anything is done to it.
fn asked(requests: usize) -> HouseholdRemoval {
    HouseholdRemoval {
        name: "ana".to_owned(),
        confirmed: false,
        requests,
        asks_through_the_request_service: true,
        revoked: Revoked::Nothing,
        findings: Vec::new(),
    }
}

/// The cost leads, and the confirmation is the last thing said.
///
/// An operator reading this is deciding, so what they lose has to be above the line
/// that tells them how to go ahead — not underneath it where they scroll past.
#[test]
fn what_goes_is_said_before_how_to_go_ahead() {
    let text = removal(&asked(2)).text();
    let cost = text.find("watch history");
    let confirm = text.find("--confirm");

    assert!(cost.is_some() && confirm.is_some(), "{text}");
    assert!(
        cost < confirm,
        "the confirmation came before the cost: {text}"
    );
    assert!(
        text.contains("Removing ana would take all of this:"),
        "{text}"
    );
}

/// Watch history is stated as a fact, because there is no option to keep it.
#[test]
fn the_watch_history_is_not_offered_as_a_choice() {
    let text = removal(&asked(0)).text();
    assert!(text.contains("It cannot be got back."), "{text}");
    assert!(
        !text.contains("kept") && !text.contains("keep"),
        "the history was described as something that could be kept: {text}"
    );
}

/// Requests stop existing rather than moving somewhere, and the number is exact.
#[test]
fn the_requests_are_counted_and_said_to_stop_existing() {
    assert!(removal(&asked(2))
        .text()
        .contains("The 2 things they asked for, which stop existing"));
    assert!(removal(&asked(1))
        .text()
        .contains("The one thing they asked for, which stops existing"));
    assert!(removal(&asked(0)).text().contains("Nothing they asked for"));
}

/// Somebody the request service never knew is said so, rather than left silent.
#[test]
fn never_having_asked_for_anything_is_said_rather_than_left_blank() {
    let never = HouseholdRemoval {
        asks_through_the_request_service: false,
        ..asked(0)
    };
    // Bound once: an argument only evaluated on failure is a line nothing runs.
    let text = removal(&never).text();
    assert!(text.contains("no account there to take"), "{text}");
    // The same report twice is the same report — held here so the equality the
    // machine-readable contract rests on is exercised rather than only derived.
    assert_eq!(never, never.clone(), "two readings of one removal differ");
}

/// Done, it says so in the past tense and names how far it got.
#[test]
fn a_confirmed_removal_says_what_it_did_and_how_far_it_reached() {
    let done = HouseholdRemoval {
        confirmed: true,
        revoked: Revoked::Everywhere,
        ..asked(1)
    };
    let text = removal(&done).text();
    assert!(
        text.contains("ana is no longer in this household."),
        "{text}"
    );
    assert!(text.contains("Both accounts are gone."), "{text}");
    assert!(text.contains("which no longer exist"), "{text}");
    assert!(
        !text.contains("--confirm"),
        "a done removal still asked: {text}"
    );
}

/// The dispatcher draws a removal, which is how every surface reaches this.
///
/// Calling the renderer directly proves what it writes; this proves the outcome
/// arrives at it, which is the half a renderer's own tests cannot see.
#[test]
fn the_dispatch_draws_a_removal() {
    let drawn = crate::render::shaped(&lemonfiber_core::app::Outcome::Removed(asked(1))).text();

    assert!(
        drawn.contains("Removing ana would take all of this:"),
        "{drawn}"
    );
}

/// Several requests read as several, and in the tense of a run that has happened.
#[test]
fn more_than_one_request_reads_as_more_than_one() {
    let done = HouseholdRemoval {
        confirmed: true,
        revoked: Revoked::Everywhere,
        ..asked(3)
    };
    let text = removal(&done).text();
    assert!(
        text.contains("The 3 things they asked for, which no longer exist"),
        "{text}"
    );
}

/// One that reached only the media server says which half is outstanding.
#[test]
fn a_partial_removal_names_the_half_that_is_left() {
    let half = HouseholdRemoval {
        confirmed: true,
        revoked: Revoked::MediaServerOnly,
        findings: vec!["the request service still holds an account".to_owned()],
        ..asked(1)
    };
    let text = removal(&half).text();
    assert!(text.contains("The request service's is not"), "{text}");
    assert!(text.contains("! the request service still holds"), "{text}");
}
