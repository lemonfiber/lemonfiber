use super::State;

#[test]
fn a_request_nobody_has_ruled_on_is_waiting_whatever_the_media_says() {
    // The request's own status settles it while it is still waiting: the media has
    // no bearing on something nobody has approved.
    for media in 1..=7 {
        assert_eq!(State::of(1, media), Some(State::WaitingForApproval));
    }
}

#[test]
fn a_refused_or_failed_request_says_so_rather_than_reading_the_media() {
    assert_eq!(State::of(3, 5), Some(State::Declined));
    assert_eq!(State::of(4, 1), Some(State::Failed));
}

#[test]
fn an_approved_request_stands_where_its_media_stands() {
    // Approved and completed both hand over to the media: the request has nothing
    // further to say once it has been let through.
    for request in [2, 5] {
        assert_eq!(State::of(request, 1), Some(State::Getting));
        assert_eq!(State::of(request, 2), Some(State::Getting));
        assert_eq!(State::of(request, 3), Some(State::Getting));
        assert_eq!(State::of(request, 4), Some(State::PartlyHere));
        assert_eq!(State::of(request, 5), Some(State::Here));
        assert_eq!(State::of(request, 7), Some(State::Gone));
    }
}

#[test]
fn a_status_this_build_does_not_know_is_not_guessed() {
    // Better an unread answer than a member told "on its way" about something that
    // will never arrive.
    assert_eq!(State::of(99, 5), None);
    assert_eq!(State::of(2, 99), None);
    // The blocklisted media status the request service never returns by default.
    assert_eq!(State::of(2, 6), None);
}

#[test]
fn every_state_reads_as_a_plain_phrase() {
    for state in [
        State::WaitingForApproval,
        State::Declined,
        State::Failed,
        State::Getting,
        State::PartlyHere,
        State::Here,
        State::Gone,
    ] {
        let phrase = state.phrase();
        assert!(!phrase.is_empty());
        assert!(phrase.chars().all(|c| c.is_ascii_lowercase() || c == ' '));
    }
}

#[test]
fn the_states_nobody_need_act_on_are_the_ones_going_well() {
    assert!(State::Here.settled());
    assert!(State::Getting.settled());
    assert!(State::PartlyHere.settled());
    assert!(!State::WaitingForApproval.settled());
    assert!(!State::Declined.settled());
    assert!(!State::Failed.settled());
    assert!(!State::Gone.settled());
}

#[test]
fn a_state_serialises_under_its_own_name() {
    assert_eq!(
        serde_json::to_string(&State::PartlyHere).unwrap_or_default(),
        r#""partly-here""#
    );
}
