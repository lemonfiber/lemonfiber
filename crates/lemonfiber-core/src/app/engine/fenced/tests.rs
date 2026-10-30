use super::held_back;
use crate::fence::Unfenced;
use crate::project::Project;
use crate::stack::compose::Action;

fn project() -> Project {
    Project {
        text: String::new(),
        topology: crate::topology::Topology::default(),
        unfenced: vec![("solver".to_owned(), Unfenced::Unaddressed)],
    }
}

#[test]
fn a_start_of_everything_holds_a_service_no_fence_can_be_written_for_back_and_says_why() {
    assert_eq!(
        held_back(&Action::Up, &project()).ok(),
        Some(vec![Unfenced::Unaddressed.why("solver")])
    );
    assert_eq!(
        held_back(&Action::Start(vec!["other".to_owned()]), &project())
            .ok()
            .map(|said| said.len()),
        Some(1)
    );
}

#[test]
fn a_start_or_restart_naming_it_is_refused_naming_it_and_why() {
    for action in [
        Action::Start(vec!["solver".to_owned()]),
        Action::Restart(vec!["other".to_owned(), "solver".to_owned()]),
    ] {
        let refused = held_back(&action, &project()).err();
        assert!(
            refused.as_ref().is_some_and(|problem| problem.code
                == crate::error::codes::stack::SERVICE_UNFENCED
                && problem.summary.contains("solver")
                && problem
                    .detail
                    .as_deref()
                    .is_some_and(|detail| detail.contains("host are not known"))),
            "{refused:?}"
        );
    }
}
