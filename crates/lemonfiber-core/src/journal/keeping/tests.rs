use super::{horizon, kept, runs, RUNS_KEPT};
use crate::journal::{Change, Kind};

/// One change of one run, stamped so that a stamp is a run.
fn change(at: usize, key: &str) -> Change {
    Change {
        at: at.to_string(),
        operation: "apply".to_owned(),
        target: ".env".to_owned(),
        kind: Kind::Set {
            key: key.to_owned(),
            previous: None,
            current: "1".to_owned(),
        },
    }
}

/// A record of `many` runs, two changes each, oldest first.
fn record(many: usize) -> Vec<Change> {
    (0..many)
        .flat_map(|at| [change(at, "PUID"), change(at, "PGID")])
        .collect()
}

#[test]
fn a_record_inside_the_bound_is_left_whole_and_says_so() {
    let held = record(3);
    assert_eq!(kept(&held).len(), held.len(), "nothing was dropped");
    assert_eq!(runs(&held), 3, "three runs, not six changes");
    let said = horizon(&held);
    assert!(
        said.contains("holds 3 of them") && said.contains("nothing has been dropped"),
        "the horizon says the record is whole: {said}"
    );
}

#[test]
fn a_record_past_the_bound_keeps_the_newest_runs_and_drops_the_oldest() {
    let held = record(RUNS_KEPT + 5);
    let left = kept(&held);

    assert_eq!(runs(left), RUNS_KEPT, "the bound is what is left");
    assert_eq!(
        left.len(),
        RUNS_KEPT * 2,
        "and every run left is whole, both of its changes"
    );
    assert_eq!(
        left.first().map(|change| change.at.as_str()),
        Some("5"),
        "the five oldest runs went, newest kept"
    );
    assert!(
        horizon(left).contains("anything older has been dropped"),
        "and the horizon stops promising the whole record"
    );
}

#[test]
fn an_empty_record_still_has_a_horizon() {
    assert!(kept(&[]).is_empty(), "nothing to keep");
    assert!(
        horizon(&[]).contains("holds 0 of them"),
        "and a machine that has changed nothing says so"
    );
}
