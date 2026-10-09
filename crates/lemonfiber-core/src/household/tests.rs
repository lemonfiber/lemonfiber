use super::State;
use crate::ports::service::{MediaStatus, RequestStatus};

const EVERY_MEDIA: [Option<MediaStatus>; 7] = [
    None,
    Some(MediaStatus::Unknown),
    Some(MediaStatus::Pending),
    Some(MediaStatus::Processing),
    Some(MediaStatus::PartlyAvailable),
    Some(MediaStatus::Available),
    Some(MediaStatus::Deleted),
];

#[test]
fn a_request_nobody_has_ruled_on_is_waiting_whatever_the_media_says() {
    // The request's own status settles it while it is still waiting: the media has
    // no bearing on something nobody has approved.
    for media in EVERY_MEDIA {
        assert_eq!(
            State::of(Some(RequestStatus::Pending), media),
            Some(State::WaitingForApproval)
        );
    }
}

#[test]
fn a_refused_or_failed_request_says_so_rather_than_reading_the_media() {
    for media in EVERY_MEDIA {
        assert_eq!(
            State::of(Some(RequestStatus::Declined), media),
            Some(State::Declined)
        );
        assert_eq!(
            State::of(Some(RequestStatus::Failed), media),
            Some(State::Failed)
        );
    }
}

#[test]
fn an_approved_request_stands_where_its_media_stands() {
    // Approved and completed both hand over to the media: the request has nothing
    // further to say once it has been let through.
    for request in [RequestStatus::Approved, RequestStatus::Completed] {
        let of = |media| State::of(Some(request), Some(media));
        assert_eq!(of(MediaStatus::Unknown), Some(State::Getting));
        assert_eq!(of(MediaStatus::Pending), Some(State::Getting));
        assert_eq!(of(MediaStatus::Processing), Some(State::Getting));
        assert_eq!(of(MediaStatus::PartlyAvailable), Some(State::PartlyHere));
        assert_eq!(of(MediaStatus::Available), Some(State::Here));
        assert_eq!(of(MediaStatus::Deleted), Some(State::Gone));
    }
}

#[test]
fn a_status_the_contract_does_not_name_is_not_guessed() {
    // Better an unread answer than a member told "on its way" about something that
    // will never arrive.
    assert_eq!(State::of(None, Some(MediaStatus::Available)), None);
    assert_eq!(State::of(Some(RequestStatus::Approved), None), None);
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
