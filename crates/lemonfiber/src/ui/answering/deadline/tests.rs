use std::time::Duration;

use axum::body::Body as Answered;
use http_body::Body as _;

use super::Deadlined;

#[tokio::test]
async fn a_body_says_what_the_body_it_wraps_says_about_its_size() {
    let promised = Deadlined::within(Answered::from("hello"), Duration::from_secs(1));
    assert_eq!(promised.size_hint().exact(), Some(5));
    assert!(!promised.is_end_stream());
}

#[tokio::test]
async fn a_body_that_is_already_over_says_so_and_holds_no_deadline() {
    let nothing = Deadlined::within(Answered::empty(), Duration::from_secs(1));
    assert!(nothing.is_end_stream());
    assert!(
        nothing.until.is_none(),
        "a body with nothing to read was given a timer"
    );
}
