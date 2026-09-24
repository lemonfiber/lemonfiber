use super::{Noticed, Silence, APART, GIVEN_UP};

/// A record that answered at this moment and read this version.
fn answered(at: u64, offered: Option<&str>) -> Noticed {
    let mut noticed = Noticed::default();
    let notes = offered.map(|offered| format!("what {offered} changed"));
    noticed.answered(at, offered.map(str::to_owned), notes, None);
    noticed
}

/// A record that has failed this many times in a row, the last at this moment.
fn silent(times: u32, at: u64) -> Noticed {
    let mut noticed = Noticed::default();
    for _ in 0..times {
        noticed.silent(at);
    }
    noticed
}

#[test]
fn a_machine_that_has_never_asked_asks() {
    assert!(Noticed::default().due(0));
    assert_eq!(Noticed::default().remembered(), None);
    assert!(!Noticed::default().given_up());
}

#[test]
fn asking_twice_in_a_row_reaches_the_network_once() {
    let noticed = answered(1_000, Some("0.13.0"));
    assert!(!noticed.due(1_000));
    assert!(!noticed.due(1_000 + APART - 1));
    assert!(noticed.due(1_000 + APART));
}

#[test]
fn what_the_last_answer_read_is_remembered_so_the_next_run_need_not_ask() {
    assert_eq!(answered(1_000, Some("0.13.0")).remembered(), Some("0.13.0"));
}

/// A check that reached the address and could make nothing of the answer still
/// counts as answered — nothing failed — and must not throw away what an earlier
/// one had read.
#[test]
fn an_answer_holding_no_version_leaves_the_one_already_known_alone() {
    let mut noticed = answered(1_000, Some("0.13.0"));
    noticed.answered(2_000, None, None, None);
    assert_eq!(noticed.remembered(), Some("0.13.0"));
    // And the notes stay with it, rather than being cleared by a check that
    // learned nothing — a version with no notes beside it would read as a
    // release that changed nothing.
    assert_eq!(noticed.changed(), Some("what 0.13.0 changed"));
    assert!(!noticed.given_up());
}

#[test]
fn each_failure_leaves_a_longer_wait_than_the_one_before() {
    let mut last = 0;
    for failures in 1..GIVEN_UP {
        let noticed = silent(failures, 0);
        let waited = (1..APART * 64)
            .find(|now| noticed.due(*now))
            .unwrap_or_default();
        assert!(
            waited > last,
            "{failures} failures waited {waited}, no longer than {last}"
        );
        last = waited;
    }
}

#[test]
fn failing_enough_times_in_a_row_stops_it_being_attempted_at_all() {
    let noticed = silent(GIVEN_UP, 0);
    assert!(noticed.given_up());
    assert!(!noticed.due(0));
    assert!(!noticed.due(APART * 1_000));
}

#[test]
fn a_check_that_answers_puts_the_wait_back_to_where_it_started() {
    let mut noticed = silent(GIVEN_UP - 1, 0);
    assert!(!noticed.due(APART));
    noticed.answered(APART * 100, Some("0.13.0".to_owned()), None, None);
    assert!(!noticed.due(APART * 100));
    assert!(noticed.due(APART * 101));
}

/// A clock that went backwards between two runs is a laptop that suspended
/// across a time-zone change, and it must not turn into a wait of decades.
#[test]
fn a_clock_that_went_backwards_is_read_as_no_time_having_passed() {
    assert!(!answered(1_000_000, Some("0.13.0")).due(1));
}

/// Each reason says which it is and what, if anything, to do about it. A single
/// "could not tell" would leave an operator who turned the check off looking for a
/// network fault.
#[test]
fn each_reason_for_not_knowing_is_a_different_thing_to_do_about_it() {
    let reasons = [
        Silence::Refused,
        Silence::GivenUp,
        Silence::Unanswered,
        Silence::NotYet,
    ];
    let said: Vec<&str> = reasons.iter().map(|reason| reason.why()).collect();
    for one in &said {
        assert!(one.split_whitespace().count() >= 20, "{one}");
    }
    let mut distinct = said.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(distinct.len(), said.len());
    assert!(Silence::Refused.why().contains("settings say so"));
    assert!(Silence::GivenUp.why().contains("stopped asking"));
}

#[test]
fn what_is_remembered_survives_being_written_down_and_read_back() {
    let noticed = answered(1_000, Some("0.13.0"));
    let written = serde_json::to_string(&noticed).unwrap_or_default();
    let read: Noticed = serde_json::from_str(&written).unwrap_or_default();
    assert_eq!(read, noticed);
}
