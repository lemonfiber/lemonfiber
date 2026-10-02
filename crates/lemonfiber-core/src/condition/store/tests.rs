use super::{Conditions, Fault};
use crate::error::Severity;

/// What a check reports, with something to do about it.
fn wrong(severity: Severity, summary: &str) -> Fault {
    Fault::new(
        "queue.stalled",
        severity,
        summary,
        "nothing that needs it is working",
        "look at it",
    )
}

/// A store with one stalled queue raised at a fixed moment. Stamps are seconds
/// since the epoch, which is what the clock port hands out.
fn stalled() -> Conditions {
    let mut conditions = Conditions::new();
    conditions.observe(
        "queue.stalled",
        Some(&wrong(Severity::Warning, "two downloads have not moved")),
        "1000",
    );
    conditions
}

/// Two different numbers about two different things: one says the problem keeps
/// coming back on its own, the other says lemonfiber keeps being wrong about it.
#[test]
fn attempts_count_repairs_that_left_the_fault_standing() {
    let mut conditions = Conditions::new();
    conditions.observe(
        "vpn.port-forward-client",
        Some(&wrong(Severity::Warning, "it is on the wrong port")),
        "1000",
    );

    conditions.attempted("vpn.port-forward-client");
    conditions.attempted("vpn.port-forward-client");
    assert_eq!(
        conditions
            .get("vpn.port-forward-client")
            .map(|condition| condition.attempts),
        Some(2)
    );

    // The fault going away is what earns the reset, not the repair having run.
    conditions.mended("vpn.port-forward-client");
    assert_eq!(
        conditions
            .get("vpn.port-forward-client")
            .map(|condition| condition.attempts),
        Some(0)
    );

    // A check nothing has recorded is not invented by counting against it.
    conditions.attempted("nothing.here");
    assert!(conditions.get("nothing.here").is_none());
}

#[test]
fn a_check_that_finds_something_wrong_raises_it() {
    let conditions = stalled();
    assert_eq!(conditions.raised().len(), 1);
    assert!(conditions
        .get("queue.stalled")
        .is_some_and(super::Condition::is_raised));
}

#[test]
fn a_standing_fault_seen_again_is_the_same_condition() {
    // Every run re-raises what is still wrong; that must land on the condition
    // already there rather than starting a second one, or "since" would mean
    // "since the last time anyone looked".
    let mut conditions = stalled();
    conditions.observe(
        "queue.stalled",
        Some(&wrong(Severity::Error, "and now the disk is full")),
        "2000",
    );
    assert_eq!(conditions.raised().len(), 1, "one problem, not two");
    let condition = conditions.get("queue.stalled");
    assert_eq!(
        condition.map(|c| (c.since.as_str(), c.severity, c.recurrences)),
        Some(("1000", Severity::Error, 0)),
        "since it broke, at the severity it has become"
    );
}

#[test]
fn a_check_that_finds_nothing_clears_what_it_raised() {
    // The self-resolving stall: it went away on its own, and that is recorded
    // rather than silently forgotten.
    let mut conditions = stalled();
    conditions.observe("queue.stalled", None, "1500");
    assert!(conditions.raised().is_empty());
    let cleared = conditions
        .get("queue.stalled")
        .and_then(|condition| condition.cleared.clone());
    assert_eq!(cleared.as_deref(), Some("1500"));
}

#[test]
fn a_healthy_check_that_never_failed_writes_nothing() {
    // Inventing a cleared condition for every passing check would fill the
    // store with things that never happened.
    let mut conditions = Conditions::new();
    conditions.observe("vpn.egress-match", None, "1000");
    assert!(conditions.is_empty());
}

#[test]
fn what_is_wrong_is_listed_worst_first_and_stably_within_a_severity() {
    let mut conditions = Conditions::new();
    for (check, severity) in [
        ("b.warn", Severity::Warning),
        ("a.critical", Severity::Critical),
        ("a.warn", Severity::Warning),
        ("c.error", Severity::Error),
    ] {
        conditions.observe(check, Some(&wrong(severity, "wrong")), "1000");
    }
    let order: Vec<&str> = conditions
        .raised()
        .iter()
        .map(|condition| condition.check.as_str())
        .collect();
    assert_eq!(order, vec!["a.critical", "c.error", "a.warn", "b.warn"]);
}

#[test]
fn a_declined_fix_is_remembered_against_its_condition() {
    let mut conditions = stalled();
    conditions.decline("queue.stalled");
    assert!(conditions
        .get("queue.stalled")
        .is_some_and(|condition| condition.declined));
    // Declining something the store has never heard of is not a failure; there
    // is simply nothing to record it against.
    conditions.decline("nothing.here");
    assert_eq!(conditions.get("nothing.here"), None);
}

#[test]
fn forgetting_a_check_removes_it_rather_than_clearing_it() {
    // A provider that has been removed should not keep reporting that it is
    // unreachable, nor leave a cleared record implying it recovered.
    let mut conditions = stalled();
    conditions.forget("queue.stalled");
    assert_eq!(conditions.get("queue.stalled"), None);
    assert!(conditions.is_empty());
}

#[test]
fn the_store_round_trips_through_its_serialised_form() {
    // It is the only memory of what was wrong before this run.
    let conditions = stalled();
    let text = serde_json::to_string(&conditions).unwrap_or_default();
    assert_eq!(
        serde_json::from_str::<Conditions>(&text).ok(),
        Some(conditions)
    );
    // And a file written before this field existed reads as an empty store
    // rather than failing the run.
    assert_eq!(
        serde_json::from_str::<Conditions>("{}").ok(),
        Some(Conditions::new())
    );
}

#[test]
fn a_copy_lays_only_what_it_changed_over_the_store_as_it_stands() {
    let read = stalled();
    let mut copy = read.clone();
    copy.observe("queue.stalled", None, "2000");
    copy.forget("never.there");

    let mut meanwhile = read;
    meanwhile.observe(
        "storage.space",
        Some(&wrong(Severity::Error, "the volume is full")),
        "1500",
    );
    let written = copy.over(meanwhile);

    assert!(written
        .get("queue.stalled")
        .is_some_and(|condition| condition.cleared.as_deref() == Some("2000")));
    assert!(written
        .get("storage.space")
        .is_some_and(|condition| condition.since == "1500"));
}

#[test]
fn a_check_a_copy_forgot_is_gone_from_what_it_writes() {
    let mut copy = stalled();
    copy.forget("queue.stalled");

    assert!(copy.over(stalled()).get("queue.stalled").is_none());
}
