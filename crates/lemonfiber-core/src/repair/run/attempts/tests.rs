use super::{Entry, Repairs, KEPT};
use crate::repair::Outcome;

fn entry(check: &str, outcome: Outcome) -> Entry {
    Entry {
        at: "1000".to_owned(),
        check: check.to_owned(),
        did: "put it right".to_owned(),
        outcome,
    }
}

/// The attempts that changed nothing are the ones worth reading when a fault keeps
/// coming back, so they are kept beside the ones that worked.
#[test]
fn what_was_tried_is_kept_whether_or_not_it_worked() {
    let mut repairs = Repairs::default();
    repairs.record(entry("vpn.port-forward-client", Outcome::FixFailed));
    repairs.record(entry("vpn.port-forward-client", Outcome::Fixed));
    repairs.record(entry("something.else", Outcome::Fixed));

    assert_eq!(repairs.entries.len(), 3);
    assert!(repairs
        .entries
        .iter()
        .any(|entry| entry.outcome == Outcome::FixFailed));
}

/// Bounded, because a record nobody prunes is a file somebody finds the hard way —
/// and the oldest go first, since what a repair did last is what explains today.
#[test]
fn the_record_keeps_the_most_recent_and_drops_the_rest() {
    let mut repairs = Repairs::default();
    for _ in 0..=KEPT {
        repairs.record(entry("vpn.port-forward-client", Outcome::Fixed));
    }
    repairs.record(entry("the.newest", Outcome::Fixed));

    assert_eq!(repairs.entries.len(), KEPT);
    assert_eq!(
        repairs.entries.last().map(|entry| entry.check.as_str()),
        Some("the.newest")
    );
}

/// It is read back long after it was written, so what it holds has to survive the
/// round trip — including which of the outcomes it was.
#[test]
fn a_record_reads_back_as_it_was_written() {
    let mut repairs = Repairs::default();
    repairs.record(entry(
        "vpn.port-forward-client",
        Outcome::Stopped {
            leaving: "half of it".to_owned(),
        },
    ));
    let written = serde_json::to_string(&repairs).unwrap_or_default();

    assert_eq!(
        serde_json::from_str::<Repairs>(&written).ok(),
        Some(repairs)
    );
}
