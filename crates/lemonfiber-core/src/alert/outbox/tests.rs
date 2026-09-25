use super::{Outbox, KEPT};
use crate::alert::{Alert, Moment};
use crate::error::Severity;

/// An alert about one check.
fn alert(check: &str, moment: Moment) -> Alert {
    Alert {
        check: check.to_owned(),
        kind: "service.stopped".to_owned(),
        moment,
        severity: Severity::Warning,
        summary: "it broke".to_owned(),
        meaning: "nothing that needs it is working".to_owned(),
        remedies: vec!["start it again".to_owned()],
        affected: vec![check.to_owned()],
    }
}

/// Every check is on its first spell.
fn first(_check: &str) -> u32 {
    0
}

#[test]
fn an_alert_is_owed_before_anything_tries_to_deliver_it() {
    // A channel that is down is exactly when something is worth saying, so
    // recording only what was sent loses the alerts that mattered most.
    let mut outbox = Outbox::new();
    outbox.owe([alert("queue.stalled", Moment::Onset)]);
    assert!(outbox.owes_anything());
    assert_eq!(outbox.owing().len(), 1);
    assert_eq!(outbox.told("queue.stalled"), None, "nothing said yet");
}

#[test]
fn delivery_moves_it_to_the_history_and_stops_it_repeating() {
    let mut outbox = Outbox::new();
    outbox.owe([alert("queue.stalled", Moment::Onset)]);
    outbox.delivered(&first);

    assert!(!outbox.owes_anything());
    assert_eq!(outbox.told("queue.stalled"), Some(0));
    assert_eq!(outbox.history().len(), 1);
}

#[test]
fn what_is_owed_is_the_current_state_and_not_every_state_it_passed_through() {
    // A channel down for an hour should not deliver forty alerts about one
    // check when it comes back; it should deliver where that check now stands.
    let mut outbox = Outbox::new();
    outbox.owe([alert("service.health", Moment::Onset)]);
    outbox.owe([alert("service.health", Moment::Resolved)]);
    assert_eq!(outbox.owing().len(), 1);
    assert_eq!(
        outbox.owing().first().map(|a| a.moment),
        Some(Moment::Resolved)
    );
}

#[test]
fn two_checks_are_two_things_owed() {
    let mut outbox = Outbox::new();
    outbox.owe([alert("a", Moment::Onset), alert("b", Moment::Onset)]);
    assert_eq!(outbox.owing().len(), 2);
}

#[test]
fn a_fault_that_came_and_went_unseen_is_still_in_the_history() {
    // The operator was away. It resolved. They can still find out it happened.
    let mut outbox = Outbox::new();
    outbox.owe([alert("disk.full", Moment::Onset)]);
    outbox.delivered(&first);
    outbox.owe([alert("disk.full", Moment::Resolved)]);
    outbox.delivered(&first);

    let history = outbox.history();
    assert_eq!(history.len(), 2);
    assert_eq!(
        history.first().map(|a| a.moment),
        Some(Moment::Resolved),
        "newest first"
    );
}

#[test]
fn the_history_is_bounded_and_keeps_the_recent_end() {
    // It is written between runs and nothing else prunes it.
    let mut outbox = Outbox::new();
    for n in 0..KEPT + 50 {
        outbox.owe([alert(&format!("check.{n}"), Moment::Onset)]);
        outbox.delivered(&first);
    }
    assert_eq!(outbox.history().len(), KEPT);
    assert_eq!(
        outbox.history().first().map(|a| a.check.as_str()),
        Some(format!("check.{}", KEPT + 49).as_str()),
        "the newest survived"
    );
}

#[test]
fn a_later_spell_is_recorded_as_a_later_spell() {
    // Delivering the second spell of a fault must not leave the outbox thinking
    // the operator has only heard about the first.
    let mut outbox = Outbox::new();
    outbox.owe([alert("queue.stalled", Moment::Onset)]);
    outbox.delivered(&|_| 2);
    assert_eq!(outbox.told("queue.stalled"), Some(2));
}

#[test]
fn forgetting_a_check_drops_what_was_owed_and_what_was_told() {
    // A provider that has been removed should not be alerted about, nor keep a
    // record implying it was.
    let mut outbox = Outbox::new();
    outbox.owe([alert("provider.quota", Moment::Onset)]);
    outbox.delivered(&first);
    outbox.owe([alert("provider.quota", Moment::Onset)]);

    outbox.forget("provider.quota");
    assert_eq!(outbox.told("provider.quota"), None);
    assert!(!outbox.owes_anything());
}

#[test]
fn the_outbox_round_trips_through_its_serialised_form() {
    // It is the only memory of what the operator has been told.
    let mut outbox = Outbox::new();
    outbox.owe([alert("a", Moment::Onset)]);
    outbox.delivered(&first);
    outbox.owe([alert("b", Moment::Onset)]);

    let text = serde_json::to_string(&outbox).unwrap_or_default();
    assert_eq!(serde_json::from_str::<Outbox>(&text).ok(), Some(outbox));
    // And a file written before any of these fields existed reads as empty.
    assert_eq!(
        serde_json::from_str::<Outbox>("{}").ok(),
        Some(Outbox::new())
    );
}
