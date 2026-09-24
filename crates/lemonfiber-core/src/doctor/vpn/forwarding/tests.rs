use super::{Answer, Forwarding};

/// What is known, given a grant and what the client says.
const fn known(granted: Option<u16>, listening: Option<u16>) -> Forwarding {
    Forwarding { granted, listening }
}

#[test]
fn the_four_questions_are_answered_separately() {
    // An operator told only that port forwarding is not working learns nothing
    // about which of these to look at.
    let matched = known(Some(51413), Some(51413));
    assert_eq!(matched.assigned(), Answer::Yes);
    assert_eq!(matched.configured(), Answer::Yes);
    assert_eq!(matched.reachable(), Answer::Unknown);
    assert_eq!(matched.unchanged(Some(51413)), Answer::Yes);
}

#[test]
fn a_client_listening_elsewhere_is_the_one_that_is_wrong() {
    // Everything else is true and nobody can reach it — the failure that looks
    // healthy from inside.
    let mismatched = known(Some(51413), Some(6881));
    assert_eq!(mismatched.assigned(), Answer::Yes);
    assert_eq!(mismatched.configured(), Answer::No);
}

#[test]
fn a_client_that_would_not_answer_settles_nothing_about_its_port() {
    // Reporting silence as a mismatch would send the operator to a setting
    // that may already be right.
    assert_eq!(known(Some(51413), None).configured(), Answer::Unknown);
    assert_eq!(known(None, Some(51413)).configured(), Answer::Unknown);
}

#[test]
fn no_grant_is_a_settled_no_rather_than_an_unknown() {
    // The provider answered; it granted nothing. That is a fact, and a
    // different one from not having been able to ask.
    assert_eq!(known(None, Some(6881)).assigned(), Answer::No);
}

#[test]
fn reachability_is_never_claimed_from_the_inside() {
    // It needs somebody on the other side of the tunnel to try the port.
    // Inferring it from a grant and a matching client would assert the very
    // thing that fails when a provider quietly stops forwarding.
    for known in [
        known(Some(51413), Some(51413)),
        known(Some(51413), Some(6881)),
        known(None, None),
    ] {
        assert_eq!(known.reachable(), Answer::Unknown);
    }
}

#[test]
fn a_reconnect_on_a_new_port_is_caught_by_comparing_with_before() {
    // A tunnel that drops and returns is commonly granted a different port,
    // and everything else goes on looking correct while the client listens on
    // yesterday's.
    assert_eq!(known(Some(51999), None).unchanged(Some(51413)), Answer::No);
    assert_eq!(known(Some(51413), None).unchanged(Some(51413)), Answer::Yes);
    // Nothing to compare with is not a change.
    assert_eq!(known(Some(51413), None).unchanged(None), Answer::Unknown);
    assert_eq!(known(None, None).unchanged(Some(51413)), Answer::Unknown);
}

#[test]
fn a_client_already_on_the_granted_port_is_left_alone() {
    // A write that changes nothing is still a write, and one made every run is
    // a client restarted every run.
    assert_eq!(known(Some(51413), Some(51413)).to_push(), None);
    assert_eq!(known(Some(51413), Some(6881)).to_push(), Some(51413));
    assert_eq!(known(Some(51413), None).to_push(), Some(51413));
    assert_eq!(known(None, Some(6881)).to_push(), None, "nothing to push");
}
