use super::{Answer, Held, Holding, Pulling, Verdict, TOLERANCE};
use crate::bandwidth::rhythm::Period;

/// A megabyte a second, which every case here is measured against.
const LIMIT: u64 = 1024 * 1024;

#[test]
fn a_client_that_took_the_limit_and_is_inside_it_is_holding() {
    let held = Held::of(Some(LIMIT), Some(LIMIT), Some(LIMIT / 2), true);
    assert_eq!(held.verdict, Verdict::Holding);
    assert!(!held.verdict.worth_saying());
    assert!(held.verdict.means().contains("accepted"));
}

#[test]
fn a_client_that_took_the_limit_and_will_not_say_what_it_is_moving_is_not_an_overrun() {
    // The throughput is a second call and it fails on its own — a client can
    // report the limit it took and then not answer about its rate at all.
    // Calling that an overrun would put a client's name in front of an
    // operator over a reading nobody took.
    let held = Held::of(Some(LIMIT), Some(LIMIT), None, true);
    assert_eq!(held.verdict, Verdict::Holding);
    assert!(!held.verdict.worth_saying());
}

#[test]
fn a_client_reporting_a_different_figure_did_not_take_the_setting() {
    // The operator is sent to the client's own configuration, which is where
    // this one is fixed.
    let held = Held::of(Some(LIMIT), Some(LIMIT * 4), Some(0), true);
    assert_eq!(held.verdict, Verdict::Ignored);
    assert!(held.verdict.means().contains("own configuration"));
    assert!(held.verdict.worth_saying());
}

#[test]
fn a_client_reporting_no_limit_at_all_did_not_take_it_either() {
    let held = Held::of(Some(LIMIT), None, Some(0), true);
    assert_eq!(held.verdict, Verdict::Ignored);
}

#[test]
fn a_client_moving_past_the_limit_it_holds_is_not_honouring_it() {
    // A different failure from the one above, and it sends the operator
    // somewhere else, so the two are never rendered alike.
    let past = LIMIT + LIMIT * TOLERANCE / 100 + 1;
    let held = Held::of(Some(LIMIT), Some(LIMIT), Some(past), true);
    assert_eq!(held.verdict, Verdict::Overrunning);
    assert!(held.verdict.means().contains("not being honoured"));
}

#[test]
fn a_rate_bouncing_around_the_limit_is_not_an_overrun() {
    // A rate is an average over the client's own window, and it sits either
    // side of the figure rather than under it. Calling every bounce an
    // overrun puts a warning on an obedient client, which is how a report
    // stops being read.
    let inside = LIMIT + LIMIT * TOLERANCE / 100;
    assert_eq!(
        Held::of(Some(LIMIT), Some(LIMIT), Some(inside), true).verdict,
        Verdict::Holding
    );
}

#[test]
fn a_limit_near_the_top_of_the_range_does_not_wrap_into_a_tiny_one() {
    assert_eq!(
        Held::of(Some(u64::MAX), Some(u64::MAX), Some(u64::MAX), true).verdict,
        Verdict::Holding
    );
}

#[test]
fn a_direction_this_client_does_not_have_is_not_a_limit_it_ignored() {
    // Usenet does not upload. Reporting that as a refused setting would send
    // an operator looking for a fault in a client that is behaving perfectly.
    let held = Held::of(Some(LIMIT), None, None, false);
    assert_eq!(held.verdict, Verdict::NothingToLimit);
    assert!(!held.verdict.worth_saying());
    assert!(held.verdict.means().contains("does not upload"));
}

#[test]
fn a_direction_nothing_was_asked_of_is_neither_holding_nor_failing() {
    let held = Held::of(None, None, Some(LIMIT), true);
    assert_eq!(held.verdict, Verdict::Unasked);
    assert!(!held.verdict.worth_saying());
    assert!(held.verdict.means().contains("nothing was asked"));
}

#[test]
fn a_client_that_could_not_be_read_is_always_worth_saying() {
    // An unknown limit rendered as no limit is a report reading better than
    // the stack is.
    let silent = Holding {
        client: "qbittorrent".to_owned(),
        answer: Answer::Silent {
            said: "connection refused".to_owned(),
        },
        pulling: None,
    };
    assert!(silent.worth_saying());
}

#[test]
fn a_client_that_has_been_stopped_is_always_worth_saying_however_well_it_holds() {
    // Its limits are perfect because it is not fetching, and a report that
    // showed only the limits would be a stack silently doing nothing.
    let stopped = Holding {
        client: "sabnzbd".to_owned(),
        answer: Answer::Held {
            down: Held::of(Some(LIMIT), Some(LIMIT), Some(0), true),
            up: Held::of(None, None, None, false),
            period: None,
        },
        pulling: Some(Pulling::Stopped),
    };
    assert!(stopped.worth_saying());
    assert!(stopped
        .pulling
        .is_some_and(|held| held.means().contains("nothing new")));

    let going = Holding {
        pulling: Some(Pulling::Fetching),
        ..stopped
    };
    assert!(!going.worth_saying());
    assert!(going.pulling.is_some_and(|held| held.means() == "fetching"));
}

#[test]
fn a_client_holding_in_both_directions_is_not_worth_interrupting_anybody_for() {
    let quiet = Holding {
        client: "qbittorrent".to_owned(),
        answer: Answer::Held {
            down: Held::of(Some(LIMIT), Some(LIMIT), Some(0), true),
            up: Held::of(Some(LIMIT), Some(LIMIT), Some(0), true),
            period: Some(Period::Quiet),
        },
        pulling: None,
    };
    assert!(!quiet.worth_saying());
}
