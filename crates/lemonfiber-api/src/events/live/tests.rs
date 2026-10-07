use std::time::Duration;

use lemonfiber_fixtures::ports::Stopped;

use lemonfiber_core::app::Outcome;
use lemonfiber_core::model::PlayingReport;

use super::Live;
use crate::events::wire::{Nature, Rendered};

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

/// A stream opened beside another hears nothing said on it, and ends when its streams
/// are told to end.
#[tokio::test]
async fn a_stream_beside_this_one_hears_none_of_it_and_ends_with_it() {
    let clock = Stopped::at(0);
    let live = Live::opening(clock.as_ref());
    let beside = live.beside(clock.as_ref());
    let mut theirs = beside.listening(None).await;

    let envelope = Outcome::Playing(PlayingReport::default()).envelope();
    live.say_if_rendered(Rendered::of(Nature::State, &envelope))
        .await;
    let heard = tokio::time::timeout(Duration::from_millis(50), theirs.next()).await;
    assert!(
        heard.is_err(),
        "a stream beside this one heard what this one said"
    );

    live.end_every_stream();
    let heard = tokio::time::timeout(Duration::from_secs(1), theirs.next()).await;
    assert_eq!(
        heard.ok(),
        Some(None),
        "a stream beside this one went on after this run's streams were ended"
    );
}
