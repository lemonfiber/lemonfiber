use super::{Alert, Moment};
use crate::condition::{Condition, Fault};
use crate::error::Severity;

/// What a check reports, with what it costs and something to do about it.
fn wrong(severity: Severity, summary: &str) -> Fault {
    Fault::new(
        "queue.stalled",
        severity,
        summary,
        "nothing is arriving for them",
        "look at it",
    )
}

/// A condition raised at a fixed moment.
fn raised() -> Condition {
    Condition::raised(
        "queue.stalled",
        &wrong(Severity::Warning, "two downloads have not moved"),
        "1000",
    )
}

#[test]
fn an_alert_carries_what_happened_what_it_means_and_what_to_do() {
    // All three or it is a notification, which is a different and worse thing:
    // an operator handed an event and an instruction has to supply the
    // judgement in between, which is the work this was supposed to save.
    let parts =
        Alert::of(&raised(), None).map(|alert| (alert.summary, alert.meaning, alert.remedies));
    assert_eq!(
        parts,
        Some((
            "two downloads have not moved".to_owned(),
            "nothing is arriving for them".to_owned(),
            vec!["look at it".to_owned()],
        ))
    );
}

#[test]
fn a_resolution_carries_the_same_three_parts_as_its_onset() {
    // Good news said in fewer parts than bad news is how the resolution reads
    // as an afterthought rather than as the other half of what was said.
    let mut condition = raised();
    condition.clear("1100");
    let told = Alert::of(&condition, Some(0)).map(|alert| {
        (
            alert.moment,
            alert.summary.is_empty(),
            alert.meaning.is_empty(),
            alert.remedies.is_empty(),
        )
    });
    assert_eq!(told, Some((Moment::Resolved, false, false, false)));
}

#[test]
fn something_newly_wrong_is_worth_an_interruption() {
    let alert = Alert::of(&raised(), None);
    assert_eq!(alert.as_ref().map(|a| a.moment), Some(Moment::Onset));
    assert_eq!(
        alert.map(|a| a.said()).as_deref(),
        Some("two downloads have not moved — started")
    );
}

#[test]
fn the_same_fault_still_wrong_says_nothing() {
    // An operator told the same thing every run stops reading, which is worse
    // than never having been told.
    assert_eq!(Alert::of(&raised(), Some(0)), None);
}

#[test]
fn a_resolution_is_news_to_whoever_heard_the_onset() {
    // Told a disk filled up and never told it was fixed, an operator goes on
    // believing it.
    let mut condition = raised();
    condition.clear("1000");
    let alert = Alert::of(&condition, Some(0));
    assert_eq!(alert.as_ref().map(|a| a.moment), Some(Moment::Resolved));
    assert_eq!(
        alert.map(|a| a.said()).as_deref(),
        Some("two downloads have not moved — resolved"),
        "and it reads as the good news it is"
    );
}

#[test]
fn a_resolution_nobody_heard_the_onset_of_says_nothing() {
    // Something that broke and fixed itself between two runs never reached them,
    // and "it is better now" about a thing they never knew was worse is noise.
    let mut condition = raised();
    condition.clear("1000");
    assert_eq!(Alert::of(&condition, None), None);
}

#[test]
fn a_resolution_carries_the_weight_of_what_resolved() {
    // "The critical thing is over" deserves the attention the critical thing had.
    let mut condition = Condition::raised(
        "vpn.leak",
        &Fault::new(
            "vpn.leak",
            Severity::Critical,
            "leaking",
            "this connection's address is visible to every peer",
            "look at it",
        ),
        "1000",
    );
    condition.clear("later");
    assert_eq!(
        Alert::of(&condition, Some(0)).map(|a| a.severity),
        Some(Severity::Critical)
    );
}

#[test]
fn only_a_critical_onset_interrupts_someone_who_asked_for_quiet() {
    let critical = Condition::raised(
        "vpn.leak",
        &Fault::new(
            "vpn.leak",
            Severity::Critical,
            "leaking",
            "this connection's address is visible to every peer",
            "look at it",
        ),
        "1000",
    );
    assert!(Alert::of(&critical, None).is_some_and(|a| a.overrides_quiet()));

    // Good news can wait for morning.
    let mut over = critical.clone();
    over.clear("later");
    assert!(Alert::of(&over, Some(0)).is_some_and(|a| !a.overrides_quiet()));

    // And a warning is not an emergency however new it is.
    assert!(Alert::of(&raised(), None).is_some_and(|a| !a.overrides_quiet()));
}
