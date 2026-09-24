use super::{Handover, Next};

#[test]
fn a_finished_walk_points_at_all_three_directions() {
    // More content, the household, and somewhere to watch — the specification's three,
    // and the only three things a household actually does next.
    assert_eq!(Handover::of(true).next, Next::all().to_vec());
    assert_eq!(Handover::default().next.len(), 3);
}

#[test]
fn a_stack_with_no_request_service_is_not_pointed_at_one() {
    // A pointer at something the operator does not have costs them the time to find
    // out it is not there.
    let handover = Handover::of(false);
    assert!(!handover.next.contains(&Next::Household));
    assert_eq!(handover.next, vec![Next::MoreContent, Next::ClientApps]);
}

#[test]
fn every_direction_says_what_it_is_and_how_to_get_there() {
    for next in Next::all() {
        assert!(!next.said().is_empty(), "{next:?}");
        assert!(!next.how().is_empty(), "{next:?}");
        assert_ne!(next.said(), next.how(), "{next:?}");
    }
}

#[test]
fn the_two_that_are_commands_are_commands() {
    assert!(Next::MoreContent.how().starts_with("lemonfiber "));
    assert!(Next::Household.how().starts_with("lemonfiber "));
    assert!(!Next::ClientApps.how().starts_with("lemonfiber "));
}
