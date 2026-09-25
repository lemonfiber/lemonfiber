use super::{control, refusal, unmeasured, unreadable, CANNOT_BE_THERE};
use std::path::Path;

/// The refusal is the useful part, and it is useful only if it names both.
#[test]
fn the_refusal_names_the_host_and_the_path_it_could_not_find() {
    let problem = refusal("nas.local", Path::new("/Volumes/media"));

    assert!(problem.summary.contains("nas.local"), "{problem:?}");
    assert!(problem.summary.contains("/Volumes/media"), "{problem:?}");
    assert!(
        problem.meaning.contains("Nothing was started"),
        "the operator is told the stack is as they left it: {}",
        problem.meaning
    );
    assert!(problem
        .remedies
        .first()
        .is_some_and(|remedy| remedy.action.contains("nas.local")));
}

/// The control question goes to the same machine, under the same location.
///
/// Somewhere else on that filesystem would be a different question: a daemon that
/// can see one tree and not another would answer "not there" for the control and
/// prove nothing about the tree the stack actually mounts.
#[test]
fn the_control_question_is_asked_under_the_location_itself() {
    let asked = control(Path::new("/srv/media"));

    assert_eq!(asked.parent(), Some(Path::new("/srv/media")));
    assert_eq!(
        asked.file_name().and_then(|name| name.to_str()),
        Some(CANNOT_BE_THERE)
    );
}

/// The two things that can go wrong send the reader to two different places, so
/// the sentences have to be told apart by reading them.
#[test]
fn a_check_that_could_not_answer_and_one_that_has_stopped_working_say_so_differently() {
    let host = "nas.local";
    let path = Path::new("/srv/media");

    let could_not = unreadable(host, path);
    let broken = unmeasured(host, path);

    assert!(could_not.contains("/srv/media") && could_not.contains(host));
    assert!(broken.contains("/srv/media") && broken.contains(host));
    assert!(
        could_not.contains("empty directory"),
        "an unverified run is told what it is risking: {could_not}"
    );
    assert!(
        broken.contains("fault in lemonfiber"),
        "the reader who can act on this one is not the operator: {broken}"
    );
    assert_ne!(could_not, broken);
}
