use std::time::Duration;

use lemonfiber_fixtures::ports::Stopped;

use super::Live;

/// A stream open when every stream is told to end, ends; one opened afterwards does not.
#[tokio::test]
async fn a_stream_open_when_they_are_ended_ends_and_a_later_one_does_not() {
    let live = Live::opening(Stopped::at(0).as_ref());
    let mut open = live.listening(None).await;

    live.end_every_stream();
    let heard = tokio::time::timeout(Duration::from_secs(1), open.next()).await;
    assert_eq!(
        heard.ok(),
        Some(None),
        "the open stream went on after it was ended"
    );

    let mut later = live.listening(None).await;
    let heard = tokio::time::timeout(Duration::from_millis(50), later.next()).await;
    assert!(
        heard.is_err(),
        "a stream opened afterwards was ended by an earlier stop"
    );
}
