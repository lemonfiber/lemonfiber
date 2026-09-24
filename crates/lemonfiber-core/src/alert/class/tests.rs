use super::Class;
use crate::error::Severity;

#[test]
fn anything_wrong_is_a_problem_whatever_it_is_called() {
    // So a new check cannot be filed as a completion and go unheard by someone
    // who asked to be told when things are wrong.
    for severity in [Severity::Warning, Severity::Error, Severity::Critical] {
        assert_eq!(Class::of("download.completed", severity), Class::Problem);
        assert_eq!(Class::of("anything.at.all", severity), Class::Problem);
    }
}

#[test]
fn work_finishing_and_work_being_available_are_different_things() {
    // Both advisory, and an operator who wants the first very often does not
    // want the second — which is the whole reason severity is not enough.
    assert_eq!(
        Class::of("download.completed", Severity::Advisory),
        Class::Completion
    );
    assert_eq!(
        Class::of("import.succeeded", Severity::Advisory),
        Class::Completion
    );
    assert_eq!(
        Class::of("update.available", Severity::Advisory),
        Class::Notice
    );
    assert_eq!(
        Class::of("backup.succeeded", Severity::Advisory),
        Class::Notice
    );
}
