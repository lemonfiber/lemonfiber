use std::collections::BTreeSet;

use super::{beside, offered, Mode, EVERY};
use crate::migration::tests::ours;
use crate::migration::Ours;

fn wanting(services: &[(&str, u16)]) -> Vec<Ours> {
    services
        .iter()
        .map(|(service, port)| ours(service, service, "1", Some(*port)))
        .collect()
}

#[test]
fn adopting_is_what_an_operator_finds_already_chosen() {
    assert_eq!(Mode::default(), Mode::Adopt);
    assert!(Mode::Adopt.preselected());
}

#[test]
fn replacing_a_working_stack_is_never_found_already_chosen() {
    assert!(!Mode::Replace.preselected());
    let preselected: Vec<&str> = EVERY
        .iter()
        .filter(|mode| mode.preselected())
        .map(|mode| mode.word())
        .collect();
    assert_eq!(preselected, vec!["adopt"], "exactly one, and it is adopt");
}

#[test]
fn replacing_is_the_only_mode_that_stops_what_is_running() {
    let disturbing: Vec<&str> = EVERY
        .iter()
        .filter(|mode| mode.disturbs_what_is_running())
        .map(|mode| mode.word())
        .collect();
    assert_eq!(disturbing, vec!["replace"]);
}

#[test]
fn every_mode_is_offered_and_says_what_it_would_come_to() {
    let read = offered();
    assert_eq!(read.len(), EVERY.len());
    assert!(read.iter().all(|mode| !mode.what.is_empty()), "{read:?}");
}

#[test]
fn the_least_destructive_mode_is_offered_first_and_the_most_last() {
    let order: Vec<String> = offered().into_iter().map(|mode| mode.mode).collect();
    assert_eq!(order.first().map(String::as_str), Some("adopt"));
    assert_eq!(order.last().map(String::as_str), Some("replace"));
}

#[test]
fn running_beside_takes_the_next_port_up_so_it_can_still_be_guessed() {
    let moved = beside(&wanting(&[("sonarr", 8989)]), &BTreeSet::new());
    let to = moved.first().map(|one| one.to);
    assert_eq!(to, Some(8990), "{moved:?}");
}

#[test]
fn a_port_something_else_answers_on_is_stepped_over() {
    let taken: BTreeSet<u16> = [8990, 8991].into_iter().collect();
    let moved = beside(&wanting(&[("sonarr", 8989)]), &taken);
    let to = moved.first().map(|one| one.to);
    assert_eq!(to, Some(8992), "{moved:?}");
}

#[test]
fn a_port_another_of_our_own_services_wants_is_stepped_over() {
    let moved = beside(
        &wanting(&[("sonarr", 8989), ("radarr", 8990)]),
        &BTreeSet::new(),
    );
    let sonarr = moved
        .iter()
        .find(|one| one.service == "sonarr")
        .map(|one| one.to);
    assert_eq!(sonarr, Some(8991), "{moved:?}");
}

#[test]
fn no_two_services_are_handed_the_same_port() {
    let moved = beside(
        &wanting(&[("sonarr", 8989), ("radarr", 8989), ("lidarr", 8989)]),
        &BTreeSet::new(),
    );
    let given: BTreeSet<u16> = moved.iter().map(|one| one.to).collect();
    assert_eq!(given.len(), moved.len(), "{moved:?}");
}

#[test]
fn a_service_with_nowhere_left_to_go_is_left_out_rather_than_given_a_bad_port() {
    let moved = beside(&wanting(&[("sonarr", u16::MAX - 1)]), &BTreeSet::new());
    assert!(moved.is_empty(), "{moved:?}");
}

#[test]
fn a_service_with_no_listener_is_not_given_a_port_to_move_to() {
    let quiet = [ours("recyclarr", "recyclarr", "1", None)];
    assert!(beside(&quiet, &BTreeSet::new()).is_empty());
}

#[test]
fn what_is_moved_reads_in_a_settled_order() {
    let moved = beside(
        &wanting(&[("radarr", 7878), ("sonarr", 8989)]),
        &BTreeSet::new(),
    );
    let order: Vec<u16> = moved.iter().map(|one| one.from).collect();
    assert_eq!(order, vec![7878, 8989]);
}
