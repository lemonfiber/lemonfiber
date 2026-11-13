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

/// A source handing over a first wave before the rest of its gather.
struct InWaves;

impl InWaves {
    /// What it says, as the dashboard would carry it.
    fn said(value: u32) -> Option<Rendered> {
        use lemonfiber_core::model::{kind, Envelope};
        Rendered::of(Nature::State, &Envelope::new(kind::DASHBOARD, value))
    }
}

#[async_trait::async_trait]
impl super::Gathers for InWaves {
    async fn gather(&self, _joined: bool) -> Vec<Rendered> {
        Self::said(202).into_iter().collect()
    }

    async fn waves(&self, _joined: bool, early: &super::Early) -> Vec<Rendered> {
        early.say(Self::said(101));
        tokio::time::sleep(Duration::from_secs(5)).await;
        self.gather(false).await
    }
}

/// What a source hands over early is heard before the rest of its gather is done,
/// and the rest follows.
#[tokio::test(start_paused = true)]
async fn what_a_source_hands_over_early_is_heard_before_the_rest_of_its_gather() {
    let live = std::sync::Arc::new(Live::opening(Stopped::at(0).as_ref()));
    let mut listening = live.listening(None).await;
    let refreshing = tokio::spawn({
        let live = std::sync::Arc::clone(&live);
        async move { live.refresh(&InWaves).await }
    });

    let first = tokio::time::timeout(Duration::from_secs(1), listening.next())
        .await
        .ok()
        .flatten()
        .unwrap_or_default();
    assert!(first.contains("101"), "the first wave, at once: {first}");
    assert!(!refreshing.is_finished(), "while the gather is still going");

    let second = tokio::time::timeout(Duration::from_secs(10), listening.next())
        .await
        .ok()
        .flatten()
        .unwrap_or_default();
    assert!(second.contains("202"), "then the rest: {second}");
}
