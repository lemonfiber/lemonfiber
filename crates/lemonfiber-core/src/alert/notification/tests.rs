use super::{Notification, RAISED};
use crate::error::Severity;

/// Anything at warning or worse is a problem, whatever it is called.
#[test]
fn anything_wrong_is_a_problem() {
    assert_eq!(
        Notification::of("download.completed", Severity::Warning),
        Notification::Problem
    );
    assert_eq!(
        Notification::of("service.stopped", Severity::Advisory),
        Notification::Problem
    );
}

/// Work finishing is a completion.
#[test]
fn work_finishing_is_a_completion() {
    assert_eq!(
        Notification::of("import.succeeded", Severity::Advisory),
        Notification::Completion
    );
}

/// The closed union reads as the contract publishes it.
#[test]
fn every_kind_is_spelled_as_the_union_publishes_it() {
    let spelled = |kind: Notification| serde_json::to_value(kind).unwrap_or_default();
    for (kind, word) in [
        (Notification::Problem, "problem"),
        (Notification::Completion, "completion"),
        (Notification::Update, "update"),
        (Notification::Backup, "backup"),
        (Notification::Request, "request"),
        (Notification::Member, "member"),
    ] {
        assert_eq!(spelled(kind), word);
    }
    assert!(RAISED.contains(&Notification::Problem));
}
