use super::{answered, disturbing, under, Widening, DIAGNOSIS, OFFERED_UNDER};
use crate::acting::question::tests::called;
use crate::acting::{Press, Stage, Wanted};
use lemonfiber_api::actions::OFFERED as WEB;
use lemonfiber_core::app::Command;
use lemonfiber_core::doctor::{Category, Narrowing};

/// The widened run one answer offers, or nothing where it offers none.
fn offered(question: &str, typed: &[&str]) -> Option<Widening> {
    let said: Vec<String> = typed.iter().map(|word| (*word).to_owned()).collect();
    under(called(question), &said)
}

/// The command one answer's offer would send.
fn sends(question: &str, typed: &[&str]) -> Option<Command> {
    offered(question, typed).map(|widening| widening.command)
}

/// Which widened run a stage is holding, or nothing for a stage holding none.
fn running(stage: &Stage) -> Option<&'static str> {
    match *stage {
        Stage::Disturbing(widened) => Some(widened.action),
        _ => None,
    }
}

/// The whole point of naming the action rather than assembling a command here:
/// every widened run has to be something another surface already offers, or the
/// requirement this screen is built for is defeated by the thing built for it.
#[test]
fn every_widened_run_is_an_action_the_other_surfaces_offer() {
    for widened in OFFERED_UNDER {
        assert!(WEB.contains(&widened.action), "{}", widened.action);
    }
    assert_eq!(OFFERED_UNDER.len(), 2);
}

/// A diagnosis asked whole is widened whole, and the widening is carried rather
/// than assumed — a run without it is the read that was just answered.
#[test]
fn a_diagnosis_asked_whole_is_widened_over_the_whole_suite() {
    assert_eq!(
        sends("how this stack is doing", &[""]),
        Some(Command::Doctor {
            narrowing: Narrowing::Suite,
            disruptive: true,
            accept: None,
        })
    );
}

/// The claim this slice turns on: the word that narrowed the reading is the word
/// that narrows the widening. An operator following a finding that says to run
/// one family would otherwise drop the tunnel to spend one indexer search.
#[test]
fn a_diagnosis_asked_about_one_family_is_widened_over_that_family_alone() {
    assert_eq!(
        sends("one family of checks", &["vpn"]),
        Some(Command::Doctor {
            narrowing: Narrowing::Category(Category::Vpn),
            disruptive: true,
            accept: None,
        })
    );
    assert_eq!(
        sends("one family of checks", &["services.releases"]),
        Some(Command::Doctor {
            narrowing: Narrowing::Check("services.releases".to_owned()),
            disruptive: true,
            accept: None,
        })
    );
}

/// Nothing this screen could not send is offered. A word the table of actions
/// refuses leaves no offer under the answer, rather than a refusal produced after
/// somebody has agreed to something.
#[test]
fn a_narrowing_that_reaches_no_command_is_not_offered_at_all() {
    assert!(sends("one family of checks", &["nonsense"]).is_none());
    // A trace with nothing typed is refused by the same table, for the reason the
    // read refuses it: a trace with no subject follows nothing.
    assert!(sends("where one thing is", &[""]).is_none());
}

/// A trace is widened over the show it followed, and the widened run is the search
/// against the indexers rather than the read that was just answered.
#[test]
fn a_trace_is_widened_into_the_search_that_asks_the_indexers() {
    assert_eq!(
        sends("where one thing is", &["The Expanse"]),
        Some(Command::Trace {
            term: "The Expanse".to_owned(),
            season: None,
            searching: true,
        })
    );
}

/// The season the reading was narrowed to narrows the search too — carried over
/// rather than dropped, or an operator asking about one season would spend the
/// search on a report about every season there is.
#[test]
fn a_trace_narrowed_to_one_season_searches_for_that_season() {
    assert_eq!(
        sends("where one season of it is", &["The Expanse", "2"]),
        Some(Command::Trace {
            term: "The Expanse".to_owned(),
            season: Some(2),
            searching: true,
        })
    );
}

/// An answer over a read that carries no widening offers nothing, or a `y` over a
/// setting would take the tunnel away.
#[test]
fn an_answer_over_a_read_with_no_widening_offers_none() {
    assert!(sends("settings", &[""]).is_none());
    assert!(sends("what to watch on", &[]).is_none());
}

/// Each question says what its own run costs before it costs it, which is the
/// whole of what an operator has to decide on.
#[test]
fn each_question_names_what_its_run_will_spend() {
    for widened in OFFERED_UNDER {
        assert!(!widened.name.is_empty(), "{}", widened.action);
        assert!(!widened.asks.is_empty(), "{}", widened.action);
        assert!(widened.about.contains("search"), "{}", widened.about);
    }
    assert!(DIAGNOSIS.asks.contains("disturb"), "{}", DIAGNOSIS.asks);
    assert!(DIAGNOSIS.about.contains("tunnel"), "{}", DIAGNOSIS.about);
}

/// Only an explicit yes goes ahead, and everything else puts the answer away.
#[test]
fn only_an_explicit_yes_runs_the_widened_diagnosis() {
    let mut stage = Stage::Idle;

    assert_eq!(
        answered(
            &mut stage,
            offered("how this stack is doing", &[""]),
            &Press::Typed('n')
        ),
        Wanted::Nothing
    );
    assert!(matches!(stage, Stage::Idle));

    let wanted = answered(
        &mut stage,
        offered("how this stack is doing", &[""]),
        &Press::Typed('Y'),
    );

    assert!(matches!(wanted, Wanted::Carry(Command::Doctor { .. })));
    assert_eq!(running(&stage), Some("diagnose"));
}

/// The yes over a trace sends the search, and the stage carries which run it is —
/// so the foot of the screen names the search rather than the checks.
#[test]
fn a_yes_over_a_trace_sends_the_search_and_says_which_run_it_is() {
    let mut stage = Stage::Idle;
    assert_eq!(
        running(&stage),
        None,
        "a screen holding nothing is holding no widened run"
    );

    let wanted = answered(
        &mut stage,
        offered("where one thing is", &["The Expanse"]),
        &Press::Typed('y'),
    );

    assert!(matches!(wanted, Wanted::Carry(Command::Trace { .. })));
    assert_eq!(running(&stage), Some("search"));
}

/// An answer with no widening under it answers nothing to a `y`.
#[test]
fn a_yes_over_an_answer_with_no_offer_under_it_sends_nothing() {
    let mut stage = Stage::Idle;

    assert_eq!(
        answered(&mut stage, None, &Press::Typed('y')),
        Wanted::Nothing
    );

    assert!(matches!(stage, Stage::Idle));
}

/// While it runs, leaving is the only thing left to ask — and everything else
/// leaves it running.
#[test]
fn a_run_that_disturbs_is_left_rather_than_stopped() {
    let mut stage = Stage::Disturbing(&DIAGNOSIS);

    assert_eq!(
        disturbing(&mut stage, &DIAGNOSIS, &Press::Forward),
        Wanted::Nothing
    );
    assert_eq!(running(&stage), Some("diagnose"));

    assert_eq!(
        disturbing(&mut stage, &DIAGNOSIS, &Press::Typed('q')),
        Wanted::Leave
    );
    assert_eq!(running(&stage), Some("diagnose"));
}
