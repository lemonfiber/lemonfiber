use lemonfiber_core::error::Came;
use lemonfiber_core::plugin::running::{Ran, StepRan};
use lemonfiber_core::plugin::{Demand, Installs, On, Recipe, RecipeInput, Step};

use super::super::installs;
use super::demanded;

/// A demand recipe that asks for a code and makes one call.
fn rescan() -> Recipe {
    Recipe {
        id: "rescan".to_owned(),
        title: "Rescan the library".to_owned(),
        why: "When files were added by hand".to_owned(),
        on: On::Demand,
        inputs: vec![RecipeInput {
            name: "code".to_owned(),
            ask: "The code Komga shows".to_owned(),
            secret: true,
        }],
        steps: vec![Step {
            id: "rescan".to_owned(),
            method: "POST".to_owned(),
            to: "komga".to_owned(),
            path: "/api/rescan".to_owned(),
            native: None,
        }],
        pairs: Vec::new(),
    }
}

/// What asking for it came to, read or run.
fn asked(ran: Option<Ran>) -> Installs {
    Installs {
        installed: Vec::new(),
        install: None,
        removal: None,
        update: None,
        substituted: Vec::new(),
        sources: Vec::new(),
        nonconforming: Vec::new(),
        demand: Some(Box::new(Demand {
            plugin: "komga".to_owned(),
            recipe: rescan(),
            ran,
        })),
        proof: None,
        agreement: Some("1a2b3c4d".to_owned()),
        rehearsed: false,
        unvetted: None,
        guards: Vec::new(),
    }
}

#[test]
fn a_reading_says_what_would_run_and_how_to_answer_it() {
    let said = installs(&asked(None)).text();
    assert!(said.contains("Would run komga's recipe rescan:"), "{said}");
    assert!(said.contains("asks code: The code Komga shows"), "{said}");
    assert!(
        said.contains("To run it, run the same command again"),
        "{said}"
    );
    assert!(said.contains("--offer 1a2b3c4d"), "{said}");
}

#[test]
fn a_run_says_what_each_step_came_to_and_asks_nothing_more() {
    let ran = Ran {
        recipe: "rescan".to_owned(),
        held: true,
        why: None,
        steps: vec![StepRan {
            step: "rescan".to_owned(),
            to: "komga".to_owned(),
            came: Came::Answered,
            status: Some(200),
            tries: 1,
            landed: false,
            why: None,
        }],
    };
    let said = installs(&asked(Some(ran))).text();
    assert!(said.contains("Ran komga's recipe rescan:"), "{said}");
    assert!(
        said.contains("rescan to komga: answered as it should"),
        "{said}"
    );
    assert!(!said.contains("--offer"), "{said}");
}

#[test]
fn every_way_a_step_can_end_is_said_in_words() {
    for came in [
        Came::Answered,
        Came::Skipped,
        Came::NotReached,
        Came::Unreachable,
        Came::Refused,
        Came::Withheld,
        Came::Unexpected,
        Came::Uncaptured,
        Came::Oversized,
    ] {
        assert!(!super::came(came).is_empty(), "{came:?}");
    }
    assert!(demanded(&Demand {
        plugin: "komga".to_owned(),
        recipe: rescan(),
        ran: None,
    })
    .text()
    .contains("runs only when asked for by name"));
}
