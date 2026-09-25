use super::{carryable, missing, unmatched};
use crate::ports::service::Carried;

fn carried(name: &str, profile: Option<&str>) -> Carried {
    Carried {
        name: name.to_owned(),
        profile: profile.map(str::to_owned),
        folder: None,
        rest: format!("{{\"title\":\"{name}\"}}"),
    }
}

#[test]
fn a_record_not_held_here_is_one_to_carry() {
    let theirs = [carried("Taskmaster", None)];
    let named: Vec<String> = missing(&theirs, &[])
        .into_iter()
        .map(|one| one.name)
        .collect();
    assert_eq!(named, vec!["Taskmaster".to_owned()]);
}

/// Overwriting what the operator has since changed here would be undoing their work
/// in the name of copying it.
#[test]
fn a_record_already_held_here_is_left_exactly_as_it_is() {
    let theirs = [carried("Taskmaster", None)];
    let ours = [carried("Taskmaster", Some("Any"))];
    assert!(missing(&theirs, &ours).is_empty());
}

#[test]
fn a_profile_this_stack_has_is_no_obstacle() {
    let carrying = [carried("Taskmaster", Some("HD"))];
    assert!(unmatched(&carrying, &["HD".to_owned()]).is_empty());
    assert_eq!(carryable(&carrying, &["HD".to_owned()]).len(), 1);
}

/// The one case worth stopping for: carried without its profile it would be
/// re-graded, which is worse than not being carried.
#[test]
fn a_profile_this_stack_lacks_stops_the_record_and_says_why() {
    let carrying = [carried("Taskmaster", Some("Bespoke"))];
    let refused = unmatched(&carrying, &["HD".to_owned()]);
    let said = refused
        .first()
        .map(|one| one.because.clone())
        .unwrap_or_default();
    assert!(said.contains("Bespoke"), "{said}");
    assert!(said.contains("re-graded"), "{said}");
    assert!(carryable(&carrying, &["HD".to_owned()]).is_empty());
}

/// An indexer follows no profile at all, and nothing about it needs one.
#[test]
fn a_record_following_no_profile_is_carried_freely() {
    let carrying = [carried("an indexer", None)];
    assert!(unmatched(&carrying, &[]).is_empty());
    assert_eq!(carryable(&carrying, &[]).len(), 1);
}

/// One bad record does not stop the rest.
#[test]
fn the_records_that_can_be_carried_are_carried() {
    let carrying = [
        carried("Taskmaster", Some("HD")),
        carried("Bake Off", Some("Bespoke")),
    ];
    let going: Vec<String> = carryable(&carrying, &["HD".to_owned()])
        .into_iter()
        .map(|one| one.name)
        .collect();
    assert_eq!(going, vec!["Taskmaster".to_owned()]);
}
