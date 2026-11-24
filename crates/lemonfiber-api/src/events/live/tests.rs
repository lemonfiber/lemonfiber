use std::time::Duration;

use lemonfiber_fixtures::ports::Stopped;

use lemonfiber_core::model::PlayingReport;

use super::Live;
use crate::events::wire::{EventKind, Rendered};

/// A stream open when every stream is told to end, ends; one opened afterwards does not.
#[tokio::test]
async fn a_stream_open_when_they_are_ended_ends_and_a_later_one_does_not() {
    let live = Live::opening(Stopped::at(0));
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
    let live = Live::opening(clock.clone());
    let beside = live.beside();
    let mut theirs = beside.listening(None).await;

    live.say_if_rendered(Rendered::of(EventKind::Playing, &PlayingReport::default()))
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

    async fn wanted_unheard(&self) -> bool {
        self.wanted.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// With nobody listening a source is gathered only once it wants to be, and being roused
/// is what starts that.
#[tokio::test]
async fn a_source_that_wants_it_is_gathered_with_nobody_listening_once_roused() {
    use std::sync::atomic::Ordering;
    let live = std::sync::Arc::new(Live::opening(Stopped::at(0)));
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
        Rendered::of(EventKind::Dashboard, &value)
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
    let live = std::sync::Arc::new(Live::opening(Stopped::at(0)));
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

/// A source handing over its first wave and finishing in the same breath.
struct AtOnce;

#[async_trait::async_trait]
impl super::Gathers for AtOnce {
    async fn gather(&self, _joined: bool) -> Vec<Rendered> {
        InWaves::said(202).into_iter().collect()
    }

    async fn waves(&self, joined: bool, early: &super::Early) -> Vec<Rendered> {
        early.say(InWaves::said(101));
        self.gather(joined).await
    }
}

/// What a source hands over early is heard before the rest even where its gather is done
/// before anything was heard.
#[tokio::test(start_paused = true)]
async fn what_is_handed_over_early_is_heard_first_even_from_a_gather_already_done() {
    let live = Live::opening(Stopped::at(0));
    let mut listening = live.listening(None).await;

    live.refresh(&AtOnce).await;

    let mut heard = Vec::new();
    for _ in 0..2 {
        heard.push(
            tokio::time::timeout(Duration::from_secs(1), listening.next())
                .await
                .ok()
                .flatten()
                .unwrap_or_default(),
        );
    }
    assert!(
        heard.first().is_some_and(|said| said.contains("101")),
        "{heard:?}"
    );
    assert!(
        heard.get(1).is_some_and(|said| said.contains("202")),
        "{heard:?}"
    );
}

/// Several sources gathered as one answer with what each says, in the order given.
#[tokio::test]
async fn sources_gathered_together_say_each_in_turn() {
    use super::Gathers as _;
    let together = crate::events::extending::Together(vec![
        std::sync::Arc::new(AtOnce),
        std::sync::Arc::new(InWaves),
    ]);

    let said = together.gather(true).await;

    assert_eq!(said.len(), 2);
    assert!(said.iter().all(|one| one.said().contains("202")));
}

/// Several sources gathered as one in waves hand over what each says early, and answer
/// with the rest.
#[tokio::test(start_paused = true)]
async fn sources_gathered_together_in_waves_hand_over_what_each_says_early() {
    use super::Gathers as _;
    let together = crate::events::extending::Together(vec![std::sync::Arc::new(InWaves)]);
    let (early, mut heard) = super::Early::channel();

    let said = together.waves(true, &early).await;

    assert!(heard.try_recv().is_ok_and(|one| one.said().contains("101")));
    assert_eq!(said.len(), 1);
    assert!(said.iter().all(|one| one.said().contains("202")));
}

/// Several sources gathered as one are wanted with nobody listening once any of them is.
#[tokio::test]
async fn sources_gathered_together_are_wanted_unheard_once_any_is() {
    use super::Gathers as _;
    let quiet = || {
        std::sync::Arc::new(Counting {
            wanted: false.into(),
            gathered: 0.into(),
        })
    };
    let wanting = std::sync::Arc::new(Counting {
        wanted: true.into(),
        gathered: 0.into(),
    });
    let none = crate::events::extending::Together(vec![quiet(), quiet()]);
    let one = crate::events::extending::Together(vec![quiet(), wanting]);

    assert!(!none.wanted_unheard().await);
    assert!(one.wanted_unheard().await);
}

/// A stream opened again is told first, outside the run's places, whether it is handed
/// every alert raised since its id, and then handed them.
#[tokio::test]
async fn a_stream_opened_again_is_told_first_whether_its_alerts_are_whole() {
    let live = Live::opening(Stopped::at(0));
    live.say_if_rendered(Rendered::of(EventKind::Log, &"heard"))
        .await;
    live.say_if_rendered(Rendered::of(EventKind::Alert, &"missed"))
        .await;

    let mut again = live.listening(Some("0-1")).await;
    assert_eq!(
        again.next().await.as_deref(),
        Some("event: replay\ndata: {\"api_version\":1,\"kind\":\"replay\",\"data\":{\"alerts\":\"whole\"}}\n\n")
    );
    let missed = again.next().await.unwrap_or_default();
    assert!(missed.starts_with("id: 0-2\nevent: alert\n"), "{missed}");
}

/// A stream opened again from another run is handed the alerts kept, said to be partial.
#[tokio::test]
async fn a_stream_opened_again_from_another_run_is_told_its_alerts_are_partial() {
    let live = Live::opening(Stopped::at(0));
    live.say_if_rendered(Rendered::of(EventKind::Alert, &"kept"))
        .await;

    let mut again = live.listening(Some("elsewhere-7")).await;
    let replay = again.next().await.unwrap_or_default();
    assert!(replay.contains(r#""alerts":"partial""#), "{replay}");
    let kept = again.next().await.unwrap_or_default();
    assert!(kept.contains(r#""data":"kept""#), "{kept}");
}

/// A stream opened afresh is told nothing of alerts, and handed none.
#[tokio::test(start_paused = true)]
async fn a_stream_opened_afresh_is_told_nothing_of_alerts() {
    let live = Live::opening(Stopped::at(0));
    live.say_if_rendered(Rendered::of(EventKind::Alert, &"before"))
        .await;

    let mut afresh = live.listening(None).await;
    assert_eq!(afresh.next().await.as_deref(), Some(super::BEAT_SAID));
}

/// What a stream keeps of alerts is aged by its own clock, read when it is opened again.
#[tokio::test]
async fn alerts_are_aged_by_the_clock_the_stream_opened_with() {
    let step = crate::events::backlog::ALERTS_FOR + Duration::from_secs(1);
    let live = Live::opening(std::sync::Arc::new(
        lemonfiber_fixtures::ports::Ticking::by(step),
    ));
    let Some(heard) = Rendered::of(EventKind::Log, &"heard") else {
        unreachable!("a log line renders");
    };
    let heard = live.say(heard).await;
    live.say_if_rendered(Rendered::of(EventKind::Alert, &"long ago"))
        .await;

    let mut again = live.listening(Some(heard.id())).await;
    let replay = again.next().await.unwrap_or_default();
    assert!(replay.contains(r#""alerts":"partial""#), "{replay}");
}

/// What one arrival tells: the member, the ask and its title.
fn arrival<'a>(
    member: &'a str,
    ask: &'a str,
    title: &'a str,
) -> lemonfiber_core::ports::Arrival<'a> {
    lemonfiber_core::ports::Arrival { member, ask, title }
}

/// Everything a stream hands over before it goes quiet.
async fn until_quiet(listening: &mut super::Listening) -> Vec<String> {
    let mut heard = Vec::new();
    while let Some(said) = listening.next().await {
        if said == super::BEAT_SAID {
            break;
        }
        heard.push(said);
    }
    heard
}

/// Each member coming back is handed what they alone were told, and nobody else's.
#[tokio::test(start_paused = true)]
async fn each_member_coming_back_is_handed_their_own_arrivals_alone() {
    use lemonfiber_core::ports::Narrator as _;
    let live = std::sync::Arc::new(Live::opening(Stopped::at(0)));
    let saying = crate::events::saying::Saying::onto(std::sync::Arc::clone(&live));
    saying
        .arrived(&arrival("ana", "music-1", "Kind of Blue"))
        .await;
    saying
        .arrived(&arrival("ben", "music-2", "Blue Train"))
        .await;

    let mut hers = live.theirs("ana").await.listening(Some("before")).await;
    let hers = until_quiet(&mut hers).await;
    let [replay, ready] = hers.as_slice() else {
        unreachable!("a replay and one alert: {hers:?}");
    };
    assert!(replay.contains(r#""alerts":"partial""#), "{replay}");
    assert!(ready.contains(r#""notification":"ready""#), "{ready}");
    assert!(ready.contains("Kind of Blue"), "{ready}");
    assert!(!ready.contains("Blue Train"), "{ready}");

    let mut his = live.theirs("ben").await.listening(Some("before")).await;
    let his = until_quiet(&mut his).await.join("");
    assert!(his.contains("Blue Train"), "{his}");
    assert!(!his.contains("Kind of Blue"), "{his}");

    let mut operators = live.listening(Some("before")).await;
    let operators = until_quiet(&mut operators).await.join("");
    assert!(!operators.contains("ready"), "{operators}");
}

/// A member nothing was said to, and one let go of, are handed no arrival.
#[tokio::test(start_paused = true)]
async fn a_member_unknown_or_let_go_of_is_handed_no_arrival() {
    use lemonfiber_core::ports::Narrator as _;
    let live = std::sync::Arc::new(Live::opening(Stopped::at(0)));
    let saying = crate::events::saying::Saying::onto(std::sync::Arc::clone(&live));
    saying
        .arrived(&arrival("ana", "music-1", "Kind of Blue"))
        .await;
    saying.removed("ana").await;

    for member in ["ana", "nobody"] {
        let mut again = live.theirs(member).await.listening(Some("before")).await;
        let heard = until_quiet(&mut again).await.join("");
        assert!(!heard.contains("event: alert"), "{member}: {heard}");
    }
}

/// A member coming back from where they got to is handed only what was said after it.
#[tokio::test(start_paused = true)]
async fn a_member_is_handed_only_the_arrivals_after_where_they_got_to() {
    use lemonfiber_core::ports::Narrator as _;
    let live = std::sync::Arc::new(Live::opening(Stopped::at(0)));
    let saying = crate::events::saying::Saying::onto(std::sync::Arc::clone(&live));
    saying
        .arrived(&arrival("ana", "music-1", "Kind of Blue"))
        .await;
    saying
        .arrived(&arrival("ana", "music-2", "Blue Train"))
        .await;

    let mut again = live.theirs("ana").await.listening(Some("0-1")).await;
    let heard = until_quiet(&mut again).await;
    let [replay, ready] = heard.as_slice() else {
        unreachable!("a replay and one alert: {heard:?}");
    };
    assert!(replay.contains(r#""alerts":"whole""#), "{replay}");
    assert!(ready.starts_with("id: 0-2\n"), "{ready}");
    assert!(ready.contains("Blue Train"), "{ready}");
}

/// Past so many members, the one asked for least lately is let go of, and the rest kept.
#[tokio::test(start_paused = true)]
async fn past_so_many_members_the_one_asked_for_least_lately_goes() {
    use lemonfiber_core::ports::Narrator as _;
    let live = std::sync::Arc::new(Live::opening(Stopped::at(0)));
    let saying = crate::events::saying::Saying::onto(std::sync::Arc::clone(&live));
    saying
        .arrived(&arrival("first", "music-1", "Kind of Blue"))
        .await;
    saying
        .arrived(&arrival("second", "music-2", "Blue Train"))
        .await;
    for nth in 2..super::MEMBERS {
        let _ = live.theirs(&format!("member-{nth}")).await;
    }
    let _ = live.theirs("first").await;
    let _ = live.theirs("one more").await;

    let mut first = live.theirs("first").await.listening(Some("before")).await;
    let first = until_quiet(&mut first).await.join("");
    assert!(first.contains("Kind of Blue"), "{first}");
    let mut second = live.theirs("second").await.listening(Some("0-1")).await;
    let second = until_quiet(&mut second).await.join("");
    assert!(second.contains(r#""alerts":"partial""#), "{second}");
    assert!(!second.contains("event: alert"), "{second}");
}

/// A member's gather stops when the stream it feeds is let go, rather than going on
/// asking the media server on their behalf after they have gone.
#[tokio::test(start_paused = true)]
async fn a_gather_stops_with_the_stream_it_feeds() {
    let asked = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let later = std::sync::Arc::clone(&asked);
    let gathering = super::Gathering(tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(1)).await;
        later.store(true, std::sync::atomic::Ordering::SeqCst);
    }));

    drop(gathering);
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(
        !asked.load(std::sync::atomic::Ordering::SeqCst),
        "the gather went on after its stream was let go"
    );
}
