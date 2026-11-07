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

/// A source counting its gathers, wanted with nobody listening while `wanted` holds.
struct Counting {
    /// Whether it is wanted.
    wanted: std::sync::atomic::AtomicBool,
    /// How many times it was gathered.
    gathered: std::sync::atomic::AtomicUsize,
}

#[async_trait::async_trait]
impl super::Gathers for Counting {
    async fn gather(&self, _joined: bool) -> Vec<Rendered> {
        self.gathered
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Vec::new()
    }

    fn wanted_unheard(&self) -> bool {
        self.wanted.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// With nobody listening a source is gathered only once it wants to be, and being roused
/// is what starts that.
#[tokio::test]
async fn a_source_that_wants_it_is_gathered_with_nobody_listening_once_roused() {
    use std::sync::atomic::Ordering;
    let live = std::sync::Arc::new(Live::opening(Stopped::at(0).as_ref()));
    let source = std::sync::Arc::new(Counting {
        wanted: false.into(),
        gathered: 0.into(),
    });
    let running = tokio::spawn(std::sync::Arc::clone(&live).gathering(source.clone()));

    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(source.gathered.load(Ordering::SeqCst), 0);

    source.wanted.store(true, Ordering::SeqCst);
    live.roused();
    let gathered = tokio::time::timeout(Duration::from_secs(5), async {
        while source.gathered.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    running.abort();
    assert!(gathered.is_ok(), "it was never gathered");
}
