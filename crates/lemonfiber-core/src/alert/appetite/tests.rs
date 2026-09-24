use super::{Appetite, Wants};
use crate::alert::Class;
use crate::error::Severity;

/// A leak, and a completed download — one of each side of the choice.
const LEAK: (&str, Severity) = ("vpn.egress.leaking", Severity::Critical);
const DONE: (&str, Severity) = ("download.completed", Severity::Advisory);
const NOTICE: (&str, Severity) = ("update.available", Severity::Advisory);

/// Whether a preset, with no exceptions, wants each of the three.
fn takes(preset: Appetite) -> (bool, bool, bool) {
    let wants = Wants::preset(preset);
    (
        wants.wants(LEAK.0, LEAK.1),
        wants.wants(DONE.0, DONE.1),
        wants.wants(NOTICE.0, NOTICE.1),
    )
}

#[test]
fn the_quiet_preset_is_the_one_nobody_has_to_choose() {
    // An operator who never revisits this is better served hearing too little
    // than learning to ignore the channel that also carries the leak.
    assert_eq!(Appetite::default_appetite(), Appetite::ProblemsOnly);
    assert_eq!(Wants::default(), Wants::preset(Appetite::ProblemsOnly));
}

#[test]
fn each_preset_takes_one_more_class_than_the_last() {
    assert_eq!(takes(Appetite::ProblemsOnly), (true, false, false));
    assert_eq!(takes(Appetite::WithCompletions), (true, true, false));
    assert_eq!(takes(Appetite::Everything), (true, true, true));
}

#[test]
fn every_preset_says_what_it_is_and_what_it_means() {
    // The choice is offered in these words; an unlabelled preset is a checklist
    // with extra steps.
    for preset in Appetite::ALL {
        assert!(!preset.label().is_empty(), "{preset:?}");
        assert!(!preset.describe().is_empty(), "{preset:?}");
    }
    assert_eq!(
        Appetite::ALL.len(),
        3,
        "three, not a list of thirteen events"
    );
}

#[test]
fn an_individual_event_can_be_switched_on_against_the_preset() {
    // The preset is a starting point, not a ceiling.
    let mut wants = Wants::preset(Appetite::ProblemsOnly);
    assert!(!wants.wants(DONE.0, DONE.1));
    wants.set(DONE.0, true);
    assert!(wants.wants(DONE.0, DONE.1));
    // And the rest of the preset is untouched by the exception.
    assert!(!wants.wants(NOTICE.0, NOTICE.1));
}

#[test]
fn an_individual_event_can_be_switched_off_against_the_preset() {
    let mut wants = Wants::preset(Appetite::Everything);
    wants.set(NOTICE.0, false);
    assert!(!wants.wants(NOTICE.0, NOTICE.1));
    assert!(wants.wants(LEAK.0, LEAK.1), "the rest still arrives");
}

#[test]
fn changing_the_preset_keeps_the_specific_answers_already_given() {
    // A broader answer is not a reason to discard the more specific ones.
    let mut wants = Wants::preset(Appetite::ProblemsOnly);
    wants.set(NOTICE.0, true);
    wants.choose(Appetite::WithCompletions);
    assert_eq!(Wants::appetite(&wants), Appetite::WithCompletions);
    assert!(wants.wants(NOTICE.0, NOTICE.1), "still asked for");
}

#[test]
fn an_exception_can_be_returned_to_the_preset() {
    let mut wants = Wants::preset(Appetite::ProblemsOnly);
    wants.set(DONE.0, true);
    wants.unset(DONE.0);
    assert!(!wants.wants(DONE.0, DONE.1));
    assert_eq!(wants.exceptions().count(), 0);
}

#[test]
fn the_exceptions_are_readable_so_a_surface_can_show_what_was_changed() {
    let mut wants = Wants::preset(Appetite::ProblemsOnly);
    wants.set(DONE.0, true);
    wants.set(NOTICE.0, false);
    let listed: Vec<(&str, bool)> = wants.exceptions().collect();
    assert_eq!(
        listed,
        vec![("download.completed", true), ("update.available", false)]
    );
}

#[test]
fn a_choice_round_trips_through_its_serialised_form() {
    // It is written between runs, so what comes back has to be what went in.
    let mut wants = Wants::preset(Appetite::WithCompletions);
    wants.set(NOTICE.0, true);
    let text = serde_json::to_string(&wants).unwrap_or_default();
    assert_eq!(serde_json::from_str::<Wants>(&text).ok(), Some(wants));
}

#[test]
fn a_choice_file_written_before_exceptions_existed_still_loads() {
    let older = r#"{"preset":"with-completions"}"#;
    assert_eq!(
        serde_json::from_str::<Wants>(older).ok(),
        Some(Wants::preset(Appetite::WithCompletions))
    );
}

#[test]
fn a_problem_is_taken_by_every_preset() {
    // However quiet the operator asked to be, something being wrong is the one
    // thing they always hear.
    for preset in Appetite::ALL {
        assert!(preset.takes(Class::Problem), "{preset:?}");
    }
}
