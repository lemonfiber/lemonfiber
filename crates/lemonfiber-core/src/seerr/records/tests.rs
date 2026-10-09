use super::{media_status, request_status};
use crate::ports::service::{MediaStatus, RequestStatus};

#[test]
fn each_documented_request_number_reads_as_its_status() {
    assert_eq!(request_status(1), Some(RequestStatus::Pending));
    assert_eq!(request_status(2), Some(RequestStatus::Approved));
    assert_eq!(request_status(3), Some(RequestStatus::Declined));
    assert_eq!(request_status(4), Some(RequestStatus::Failed));
    assert_eq!(request_status(5), Some(RequestStatus::Completed));
}

#[test]
fn each_documented_media_number_reads_as_its_status() {
    assert_eq!(media_status(1), Some(MediaStatus::Unknown));
    assert_eq!(media_status(2), Some(MediaStatus::Pending));
    assert_eq!(media_status(3), Some(MediaStatus::Processing));
    assert_eq!(media_status(4), Some(MediaStatus::PartlyAvailable));
    assert_eq!(media_status(5), Some(MediaStatus::Available));
    assert_eq!(media_status(7), Some(MediaStatus::Deleted));
}

/// A number the service does not document, or the blocklisted media status it never
/// returns by default, is not guessed into the nearest status.
#[test]
fn an_undocumented_number_is_no_status() {
    for unread in [0, 6, 99] {
        assert_eq!(request_status(unread), None, "{unread}");
    }
    for unread in [0, 6, 8, 99] {
        assert_eq!(media_status(unread), None, "{unread}");
    }
}
