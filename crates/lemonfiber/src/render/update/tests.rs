use super::{back, size, update};
use lemonfiber_core::app::update::Report;
use lemonfiber_core::migration::version::Jump;
use lemonfiber_core::model::StackEdit;
use lemonfiber_core::update::{Applied, Change, Ending, Reversal};

/// One step, from `current` onto `target`.
fn change(jump: Jump, refused: bool) -> Change {
    Change {
        service: "sonarr".to_owned(),
        current: "4.0.15".to_owned(),
        target: "5.0.0".to_owned(),
        jump,
        irreversible: !refused,
        refused,
        because: "it migrates its state on first start".to_owned(),
    }
}

/// A report carrying `changes` and `applied`, agreed to or not.
fn report(changes: Vec<Change>, applied: Vec<Applied>, confirmed: bool) -> Report {
    Report {
        state: lemonfiber_core::update::state(&changes, &applied),
        changes,
        in_flight: Vec::new(),
        confirmed,
        backup: confirmed.then(|| "/tmp/one.tar.gz".to_owned()),
        stack_edits: Vec::new(),
        applied,
        halted: None,
        changelog: crate::render::fixtures::notes("0.4.0"),
    }
}

/// Driven through the whole funnel rather than through this module's own
/// entry, so the outcome the core hands over is proved to reach these lines
/// rather than only to have a rendering somewhere.
#[test]
fn a_stack_on_every_pin_is_told_once_and_asked_nothing() {
    let outcome = lemonfiber_core::app::Outcome::Update(report(Vec::new(), Vec::new(), false));
    let said = crate::render::shaped(&outcome).text();
    assert!(said.contains("Every service is on the version"), "{said}");
    assert!(!said.contains("--confirm"), "{said}");
}

#[test]
fn an_available_update_names_both_versions_and_how_to_take_it() {
    let said = update(&report(vec![change(Jump::Minor, false)], Vec::new(), false)).text();
    assert!(said.contains("4.0.15 to 5.0.0"), "{said}");
    assert!(said.contains("lemonfiber update --confirm"), "{said}");
}

#[test]
fn a_first_number_change_is_called_out_rather_than_left_to_the_numbers() {
    let major = update(&report(vec![change(Jump::Major, false)], Vec::new(), false)).text();
    let minor = update(&report(vec![change(Jump::Minor, false)], Vec::new(), false)).text();
    assert!(major.contains("A first-number change"), "{major}");
    assert!(!minor.contains("A first-number change"), "{minor}");
}

#[test]
fn a_step_lemonfiber_will_not_take_says_so_instead_of_warning_about_it() {
    let said = update(&report(vec![change(Jump::Patch, true)], Vec::new(), false)).text();
    assert!(said.contains("Refused:"), "{said}");
}

#[test]
fn each_size_of_step_has_a_word_of_its_own() {
    assert_eq!(size(Jump::Major), "major");
    assert_eq!(size(Jump::Minor), "minor");
    assert_eq!(size(Jump::Patch), "patch");
    assert_eq!(size(Jump::Untellable), "size unknown");
}

#[test]
fn a_run_says_where_the_backup_went_and_what_each_service_came_to() {
    let taken = change(Jump::Minor, false);
    let applied = vec![
        Applied::ended(
            &taken,
            Ending::NotStarted,
            Some("it did not start".to_owned()),
        ),
        Applied::ended(&taken, Ending::NotReached, None),
    ];
    let mut carried = report(vec![taken], applied, true);
    carried.halted = Some("sonarr did not come back".to_owned());
    carried.stack_edits = vec![StackEdit {
        path: "compose.yml".to_owned(),
        diff: "-yours".to_owned(),
    }];
    let said = update(&carried).text();
    assert!(said.contains("Backed up to /tmp/one.tar.gz"), "{said}");
    assert!(said.contains("started and did not come back"), "{said}");
    assert!(said.contains("not reached"), "{said}");
    assert!(said.contains("sonarr did not come back"), "{said}");
    assert!(said.contains("compose.yml is yours"), "{said}");
}

#[test]
fn a_run_where_everything_moved_says_so_in_its_first_line() {
    let taken = change(Jump::Patch, false);
    let applied = vec![Applied::ended(&taken, Ending::Updated, None)];
    let said = update(&report(vec![taken], applied, true)).text();
    assert!(said.starts_with("Updated:"), "{said}");
}

#[test]
fn a_run_that_moved_some_of_them_is_told_apart_from_one_that_moved_none() {
    let taken = change(Jump::Patch, false);
    let some = vec![
        Applied::ended(&taken, Ending::Updated, None),
        Applied::ended(&taken, Ending::NotReached, None),
    ];
    let partly = update(&report(vec![taken.clone()], some, true)).text();
    let none = update(&report(
        vec![taken.clone()],
        vec![Applied::ended(&taken, Ending::NotFetched, None)],
        true,
    ))
    .text();
    assert!(partly.starts_with("Partly updated"), "{partly}");
    assert!(none.starts_with("Nothing was updated"), "{none}");
}

#[test]
fn what_is_still_coming_down_is_named_with_the_way_to_let_it_finish() {
    let mut waiting = report(vec![change(Jump::Patch, false)], Vec::new(), false);
    waiting.in_flight = vec!["Ubuntu.iso (94%)".to_owned()];
    let said = update(&waiting).text();
    assert!(said.contains("Ubuntu.iso (94%)"), "{said}");
    assert!(said.contains("--confirm --wait"), "{said}");
}

#[test]
fn the_way_back_is_the_one_that_can_work_for_how_the_service_ended() {
    assert!(back(Reversal::Rollback).contains("starting it again"));
    assert!(back(Reversal::Restore).contains("lemonfiber restore"));
}
