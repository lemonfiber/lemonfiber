use super::{changes, state, Applied, Change, Ending, Reversal, State};
use crate::migration::tests::{image, ours};
use crate::migration::version::Jump;

/// What updating one service, standing on `running` and pinned at `pinned`, comes
/// to — or nothing where that service would not move.
fn moving(running: &str, pinned: &str) -> Option<Change> {
    let images = [image(
        &[&format!("lscr.io/linuxserver/sonarr:{running}")],
        &["lemonfiber"],
    )];
    let pins = [ours("sonarr", "lscr.io/linuxserver/sonarr", pinned, None)];
    changes(&pins, &images, "lemonfiber").into_iter().next()
}

#[test]
fn a_service_standing_on_its_pin_is_not_a_change_at_all() {
    assert_eq!(moving("4.0.15", "4.0.15"), None);
}

#[test]
fn an_available_update_names_both_versions_and_the_size_of_the_step() {
    let change = moving("4.0.15", "4.1.0");
    let read = change.map(|one| (one.current, one.target, one.jump, one.irreversible));
    assert_eq!(
        read,
        Some(("4.0.15".to_owned(), "4.1.0".to_owned(), Jump::Minor, true))
    );
}

#[test]
fn a_major_step_is_told_apart_from_the_two_below_it() {
    let major = moving("4.0.15", "5.0.0").map(|one| one.jump);
    let patch = moving("4.0.15", "4.0.16").map(|one| one.jump);
    assert_eq!(major, Some(Jump::Major));
    assert_eq!(patch, Some(Jump::Patch));
}

#[test]
fn the_step_says_it_cannot_be_walked_back_before_it_is_taken() {
    let said = moving("4.0.15", "4.1.0")
        .map(|one| one.because)
        .unwrap_or_default();
    assert!(said.contains("4.0.15"), "{said}");
    assert!(said.contains("not possible"), "{said}");
}

#[test]
fn a_pin_older_than_what_is_running_is_refused_rather_than_attempted() {
    let change = moving("4.1.0", "4.0.15");
    let read = change.map(|one| (one.refused, one.irreversible));
    assert_eq!(read, Some((true, false)));
}

#[test]
fn versions_that_cannot_be_ordered_are_backed_up_rather_than_assumed_safe() {
    let change = moving("release-0.14.5", "0.14.6");
    let read = change.map(|one| (one.refused, one.irreversible, one.jump));
    assert_eq!(read, Some((false, true, Jump::Untellable)));
}

#[test]
fn a_service_the_engine_has_no_image_for_is_not_something_to_update() {
    let images = [image(
        &["lscr.io/linuxserver/sonarr:4.0.15"],
        &["somebody-else"],
    )];
    let pins = [ours("sonarr", "lscr.io/linuxserver/sonarr", "4.1.0", None)];
    assert!(changes(&pins, &images, "lemonfiber").is_empty());
}

#[test]
fn what_would_change_reads_in_a_settled_order() {
    let images = [
        image(&["lscr.io/linuxserver/sonarr:4.0.15"], &["lemonfiber"]),
        image(&["lscr.io/linuxserver/radarr:5.0.1"], &["lemonfiber"]),
    ];
    let pins = [
        ours("sonarr", "lscr.io/linuxserver/sonarr", "4.1.0", None),
        ours("radarr", "lscr.io/linuxserver/radarr", "5.1.0", None),
    ];
    let order: Vec<String> = changes(&pins, &images, "lemonfiber")
        .into_iter()
        .map(|one| one.service)
        .collect();
    assert_eq!(order, vec!["radarr".to_owned(), "sonarr".to_owned()]);
}

#[test]
fn a_service_that_never_ran_the_new_image_is_rolled_back_rather_than_restored() {
    assert_eq!(Ending::NotFetched.reversal(), Reversal::Rollback);
    assert_eq!(Ending::NotReached.reversal(), Reversal::Rollback);
}

#[test]
fn a_service_that_started_the_new_image_asks_for_the_backup_either_way() {
    assert_eq!(Ending::Updated.reversal(), Reversal::Restore);
    assert_eq!(Ending::NotStarted.reversal(), Reversal::Restore);
}

#[test]
fn what_one_service_came_to_carries_the_reversal_its_ending_allows() {
    let change = moving("4.0.15", "4.1.0");
    let applied = change.map(|one| Applied::ended(&one, Ending::NotStarted, Some("no".to_owned())));
    let read = applied.map(|one| (one.from, one.to, one.reversal, one.detail));
    assert_eq!(
        read,
        Some((
            "4.0.15".to_owned(),
            "4.1.0".to_owned(),
            Reversal::Restore,
            Some("no".to_owned())
        ))
    );
}

/// One service that ended `ending`, for the state readings below.
fn ended(service: &str, ending: Ending) -> Applied {
    Applied {
        service: service.to_owned(),
        from: "1.0".to_owned(),
        to: "2.0".to_owned(),
        ending,
        reversal: ending.reversal(),
        detail: None,
    }
}

#[test]
fn a_stack_on_every_pin_is_current_and_a_stack_off_one_has_updates() {
    let change = moving("4.0.15", "4.1.0");
    let waiting = change.map(|one| state(&[one], &[]));
    assert_eq!(state(&[], &[]), State::Current);
    assert_eq!(waiting, Some(State::UpdatesAvailable));
}

#[test]
fn a_run_is_told_apart_by_how_many_of_its_services_took_the_update() {
    let took = [ended("sonarr", Ending::Updated)];
    let some = [
        ended("sonarr", Ending::Updated),
        ended("radarr", Ending::NotStarted),
    ];
    let none = [ended("sonarr", Ending::NotFetched)];
    assert_eq!(state(&[], &took), State::Updated);
    assert_eq!(state(&[], &some), State::Partial);
    assert_eq!(state(&[], &none), State::Failed);
}

#[test]
fn what_a_run_came_to_reads_the_same_way_however_it_is_carried() {
    let one = ended("sonarr", Ending::Updated);
    assert_eq!(one.clone(), one);
    let written = serde_json::to_string(&State::Partial).ok();
    assert_eq!(written.as_deref(), Some("\"partial\""));
    let ending = serde_json::to_string(&Ending::NotReached).ok();
    assert_eq!(ending.as_deref(), Some("\"not-reached\""));
    let reversal = serde_json::to_string(&Reversal::Rollback).ok();
    assert_eq!(reversal.as_deref(), Some("\"rollback\""));
}
