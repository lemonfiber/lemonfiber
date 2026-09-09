//! How much of the record is kept, and what that leaves readable.
//!
//! The journal is appended to for the life of a machine and read whole on every history
//! and every recovery, so something has to bound it. What a bound costs is the oldest
//! changes, which is why it is stated wherever the record is read rather than left for an
//! operator to discover by looking for a change that is no longer there.

use super::Change;

/// How many runs of the record are kept.
///
/// Counted in runs rather than in changes because a run is the unit a reversal takes. A
/// bound that cut one in half would leave changes whose siblings are gone — classified as
/// reversible and not reversible in fact. A hundred covers years of ordinary maintenance
/// on one machine and keeps a file that is read whole, twice over, small enough to read in
/// one go.
pub const RUNS_KEPT: usize = 100;

/// The run a change belongs to.
///
/// The operation names the kind of run and every run of that kind reuses it; the surface
/// stamps one time for a whole run. The pair is what tells two of them apart.
fn run(change: &Change) -> (&str, &str) {
    (&change.operation, &change.at)
}

/// How many runs the record holds.
#[must_use]
pub fn runs(changes: &[Change]) -> usize {
    let mut seen: Vec<(&str, &str)> = Vec::new();
    for change in changes {
        let run = run(change);
        if !seen.contains(&run) {
            seen.push(run);
        }
    }
    seen.len()
}

/// The changes still on record once the bound is applied, oldest first.
///
/// Walked from the newest end, so what survives is the most recent runs whole. A record
/// already within the bound is returned untouched.
#[must_use]
pub fn kept(changes: &[Change]) -> &[Change] {
    let mut seen: Vec<(&str, &str)> = Vec::new();
    let mut cut = 0;
    for (index, change) in changes.iter().enumerate().rev() {
        let run = run(change);
        if seen.contains(&run) {
            continue;
        }
        if seen.len() == RUNS_KEPT {
            cut = index + 1;
            break;
        }
        seen.push(run);
    }
    changes.get(cut..).unwrap_or_default()
}

/// How far back the record goes, as an operator reads it.
///
/// Said whether or not anything has been dropped, because a record that has been trimmed
/// and one that has always been short are the same list otherwise. A machine still inside
/// the bound is told so in as many words: what it is looking at is everything.
#[must_use]
pub fn horizon(changes: &[Change]) -> String {
    let held = runs(changes);
    if held < RUNS_KEPT {
        format!(
            "the last {RUNS_KEPT} runs of lemonfiber's own changes are kept; this record \
             holds {held} of them, so nothing has been dropped"
        )
    } else {
        format!(
            "the last {RUNS_KEPT} runs of lemonfiber's own changes; anything older has \
             been dropped"
        )
    }
}

#[cfg(test)]
mod tests {
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
}
