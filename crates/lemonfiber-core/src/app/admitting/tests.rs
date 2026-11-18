use std::time::Duration;

use super::refused::{
    held, no_session, not_open, not_the_password, not_waiting, not_yours, short_choice,
    signs_in_by_password, too_many_codes, unconfirmed,
};
use super::REFUSALS;
use crate::error::codes::admit::{CODE_NOT_WAITING, NOT_OPEN, NOT_THE_PASSWORD};
use crate::error::{Problem, Severity};

fn every() -> Vec<Problem> {
    [
        *not_yours(),
        *unconfirmed(),
        *not_the_password(),
        *held(Duration::from_secs(8)),
        *not_open(),
        *short_choice(),
        *not_waiting(),
        *too_many_codes(Duration::from_secs(8)),
        *signs_in_by_password(),
        *no_session(),
    ]
    .into()
}

#[test]
fn every_refusal_is_listed_once() {
    let codes: Vec<_> = every().iter().map(|problem| problem.code).collect();
    assert_eq!(codes, REFUSALS.to_vec());
}

#[test]
fn only_the_door_answers_unauthorized() {
    let unauthorized: Vec<_> = every()
        .iter()
        .filter(|problem| problem.status() == 401)
        .map(|problem| problem.code)
        .collect();
    assert_eq!(
        unauthorized,
        vec![NOT_THE_PASSWORD, NOT_OPEN, CODE_NOT_WAITING]
    );
}

#[test]
fn only_the_refusals_that_are_not_faults_are_warnings() {
    let warning: Vec<_> = every()
        .iter()
        .filter(|problem| problem.severity == Severity::Warning)
        .map(|problem| problem.code)
        .collect();
    assert_eq!(
        warning,
        vec![
            REFUSALS[0],
            REFUSALS[1],
            held(Duration::ZERO).code,
            too_many_codes(Duration::ZERO).code,
        ]
    );
}

#[test]
fn a_wait_is_said_in_whole_seconds_never_less_than_one() {
    assert_eq!(held(Duration::from_millis(4_900)).retry_after, Some(4));
    assert_eq!(held(Duration::ZERO).retry_after, Some(1));
    assert!(held(Duration::from_secs(64)).summary.contains("64 seconds"));
    assert_eq!(
        too_many_codes(Duration::from_secs(90)).retry_after,
        Some(90)
    );
}

#[test]
fn a_refusal_with_no_wait_says_none() {
    assert!(every()
        .iter()
        .filter(|problem| problem.status() != 429)
        .all(|problem| problem.retry_after.is_none()));
}

#[test]
fn what_a_request_carried_and_what_was_offered_are_debugged_without_their_secrets() {
    let carried = super::Carried {
        offered: Some("s3cr3t".to_owned()),
        arrived: None,
    };
    let said = format!("{carried:?}");
    assert!(
        said.contains("withheld") && !said.contains("s3cr3t"),
        "{said}"
    );
    let offered = super::Offered {
        name: Some("ana".to_owned()),
        claim: Some("cl41m".to_owned()),
        peer: None,
    };
    let said = format!("{offered:?}");
    assert!(
        said.contains("claiming: true") && !said.contains("cl41m"),
        "{said}"
    );
}
