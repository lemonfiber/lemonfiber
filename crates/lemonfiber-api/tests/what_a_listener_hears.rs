//! What a client that holds the stream open is told, and when.
//!
//! Driven from outside the crate, because an asynchronous path exercised from
//! inside it is one no coverage view can account for. Time is moved rather than
//! waited on: a fifteen-second beat has to be proved, and a test that waits a
//! quarter of a minute for it is a test somebody eventually deletes.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, Response, StatusCode};
use futures_util::StreamExt;
use lemonfiber_api::events::dashboard::Dashboard;
use lemonfiber_api::events::live::{Gathers, Live, TICK};
use lemonfiber_api::events::wire::{Event, Nature, Rendered, BEAT, BEAT_SAID};
use lemonfiber_api::events::{routes, stream, Streaming, LAST_EVENT_ID, PATH};
use lemonfiber_api::guard::{Token, TOKEN_HEADER};
use lemonfiber_core::app::Ctx;
use lemonfiber_core::config::Settings;
use lemonfiber_core::model::{kind, Envelope};
use lemonfiber_core::platform::Environment;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::files::Files;
use lemonfiber_fixtures::http::Fake;
use lemonfiber_fixtures::ports::{Chance, Idle, Stopped};
use lemonfiber_fixtures::support::Reporting;
use tokio::time::Instant;

/// Bytes this run's token is minted from.
///
/// Cycled to whatever width is asked for: a source that answers short mints no
/// token at all, and every listener here would be refused rather than answered.
fn given() -> Chance {
    Chance::cycling()
}

/// More than a listener may fall behind by before it is cut loose.
const FLOOD: usize = 200;

/// A source that says which gather this was, and counts how often it was asked.
struct Counting(AtomicUsize);

#[async_trait]
impl Gathers for Counting {
    async fn gather(&self) -> Option<Rendered> {
        let at = self.0.fetch_add(1, Ordering::SeqCst);
        Rendered::of(Nature::State, &Envelope::new(kind::DASHBOARD, at))
    }
}

/// A source with nothing to say, which must leave the stream silent rather than
/// make it say something that was not gathered.
struct Mute;

#[async_trait]
impl Gathers for Mute {
    async fn gather(&self) -> Option<Rendered> {
        None
    }
}

/// One server, and the stream it is holding open.
struct Serving {
    /// What answering a request takes, where the token could be minted.
    streaming: Option<Arc<Streaming>>,
    /// The one gather everyone listening hears.
    live: Arc<Live>,
}

impl Serving {
    /// A server opening now, with a token a test can carry.
    fn opening() -> Self {
        let live = Arc::new(Live::opening(Stopped::at(0).as_ref()));
        let streaming = Token::mint(&given()).map(|token| {
            Arc::new(Streaming {
                token,
                bound: ([127, 0, 0, 1], 8471).into(),
                live: Arc::clone(&live),
            })
        });
        Self { streaming, live }
    }

    /// This run's token, as a caller must send it back.
    fn token(&self) -> String {
        self.streaming
            .as_ref()
            .map_or_else(String::new, |streaming| streaming.token.as_str().to_owned())
    }

    /// What a request saying these headers is answered with.
    async fn answering(&self, said: &[(&str, &str)]) -> Option<Response<Body>> {
        let streaming = self.streaming.clone()?;
        Some(stream(State(streaming), saying(said)).await)
    }
}

/// A request saying what a browser on this machine would say.
fn saying(said: &[(&str, &str)]) -> HeaderMap {
    let mut headers = HeaderMap::new();
    for (name, value) in said {
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(name.to_lowercase().as_bytes()),
            HeaderValue::from_str(value),
        ) {
            headers.insert(name, value);
        }
    }
    headers
}

/// What a listener from this machine carrying this run's token says.
fn welcome(token: &str) -> Vec<(&str, &str)> {
    vec![(TOKEN_HEADER, token), ("host", "localhost:8471")]
}

/// Say one thing to everyone listening.
async fn say(live: &Live, what: &'static str) -> Option<Event> {
    let said = Rendered::of(Nature::Record, &Envelope::new(kind::LOG, what))?;
    Some(live.say(said).await)
}

/// The first thing that comes down an answered stream.
async fn first(answer: Option<Response<Body>>) -> String {
    let Some(answer) = answer else {
        return String::new();
    };
    answer
        .into_body()
        .into_data_stream()
        .next()
        .await
        .and_then(Result::ok)
        .map(|said| String::from_utf8_lossy(&said).into_owned())
        .unwrap_or_default()
}

/// What a response says it is.
fn typed(answer: Option<&Response<Body>>) -> Option<&HeaderValue> {
    answer.and_then(|answer| answer.headers().get(header::CONTENT_TYPE))
}

/// A context reaching nothing: no engine, no stack, no service answering.
///
/// The gather degrades rather than failing, so this produces a whole snapshot
/// with every panel marked — which is the point, since what is under test is
/// that it is *this* gather and not another.
fn nowhere() -> Ctx {
    Ctx::new(
        Arc::new(Idle),
        Arc::new(Reporting::absent()),
        Stopped::at(0),
        Files::empty(),
        Source::External(Path::new("/lemonfiber/no/such/stack")),
        Settings::default(),
        Environment::MacOs,
    )
    .with_http(Fake::silent())
}

#[test]
fn the_stream_is_where_both_sides_agreed_it_would_be() {
    assert_eq!(PATH, "/api/events");
    assert_eq!(LAST_EVENT_ID, "Last-Event-ID");
}

#[tokio::test]
async fn the_route_is_one_the_surface_can_merge() {
    assert!(Serving::opening().streaming.map(routes).is_some());
}

#[tokio::test]
async fn a_request_carrying_no_token_is_refused_here_as_anywhere() {
    let serving = Serving::opening();

    let answer = serving.answering(&[("host", "localhost:8471")]).await;

    assert_eq!(
        answer.as_ref().map(Response::status),
        Some(StatusCode::FORBIDDEN)
    );
    assert_eq!(
        typed(answer.as_ref()),
        Some(&HeaderValue::from_static("text/plain; charset=utf-8")),
        "a refusal is answered with, not streamed"
    );
}

#[tokio::test]
async fn a_request_naming_another_address_is_refused_here_as_anywhere() {
    let serving = Serving::opening();

    let answer = serving
        .answering(&[
            (TOKEN_HEADER, &serving.token()),
            ("host", "example.com:8471"),
        ])
        .await;

    assert_eq!(
        answer.map(|answer| answer.status()),
        Some(StatusCode::FORBIDDEN)
    );
}

#[tokio::test]
async fn an_admitted_listener_is_answered_with_a_stream_it_can_hold_open() {
    let serving = Serving::opening();

    let answer = serving.answering(&welcome(&serving.token())).await;

    assert_eq!(answer.as_ref().map(Response::status), Some(StatusCode::OK));
    assert_eq!(
        typed(answer.as_ref()),
        Some(&HeaderValue::from_static("text/event-stream"))
    );
    assert_eq!(
        answer
            .as_ref()
            .and_then(|answer| answer.headers().get(header::CACHE_CONTROL)),
        Some(&HeaderValue::from_static("no-store"))
    );
}

#[tokio::test]
async fn a_listener_hears_what_is_said_after_it_arrives() {
    let serving = Serving::opening();
    let answer = serving.answering(&welcome(&serving.token())).await;

    say(&serving.live, "a service restarted").await;

    let heard = first(answer).await;
    assert!(heard.contains("\nevent: log\n"), "{heard}");
    assert!(heard.contains("a service restarted"), "{heard}");
    assert!(heard.starts_with("id: 0-1\n"), "{heard}");
}

#[tokio::test]
async fn a_listener_that_comes_back_is_handed_the_record_it_missed_first() {
    let serving = Serving::opening();
    let seen = say(&serving.live, "before the gap").await;
    say(&serving.live, "during the gap").await;

    let token = serving.token();
    let mut carried = welcome(&token);
    let id = seen.as_ref().map(Event::id).unwrap_or_default();
    carried.push((LAST_EVENT_ID, id));
    let heard = first(serving.answering(&carried).await).await;

    assert!(heard.contains("during the gap"), "{heard}");
    assert!(!heard.contains("before the gap"), "{heard}");
}

#[tokio::test(start_paused = true)]
async fn a_stream_that_has_said_nothing_says_so_within_fifteen_seconds() {
    let live = Live::opening(Stopped::at(0).as_ref());
    let mut listening = live.listening(None).await;

    let began = Instant::now();
    let heard = listening.next().await;

    assert_eq!(heard.as_deref(), Some(BEAT_SAID));
    assert_eq!(
        began.elapsed(),
        BEAT,
        "no later, so a client can trust twice it"
    );
}

#[tokio::test(start_paused = true)]
async fn a_stream_that_has_just_spoken_waits_the_whole_interval_again() {
    let live = Live::opening(Stopped::at(0).as_ref());
    let mut listening = live.listening(None).await;
    say(&live, "a service restarted").await;

    let spoken = listening.next().await;
    let began = Instant::now();
    let beat = listening.next().await;

    assert!(spoken.is_some_and(|said| said.contains("event: log")));
    assert_eq!(beat.as_deref(), Some(BEAT_SAID));
    assert_eq!(began.elapsed(), BEAT, "a busy stream beats only once quiet");
}

#[tokio::test(start_paused = true)]
async fn a_listener_that_cannot_keep_up_is_ended_rather_than_told_less_than_it_missed() {
    let live = Live::opening(Stopped::at(0).as_ref());
    let mut listening = live.listening(None).await;
    for _ in 0..FLOOD {
        say(&live, "another one").await;
    }

    assert!(
        listening.next().await.is_none(),
        "it comes back saying where it got to, which the backlog can answer"
    );
}

#[tokio::test(start_paused = true)]
async fn a_stream_that_has_ended_ends_the_listener_with_it() {
    let live = Live::opening(Stopped::at(0).as_ref());
    let mut listening = live.listening(None).await;
    drop(live);

    assert!(listening.next().await.is_none());
}

#[tokio::test(start_paused = true)]
async fn a_source_with_nothing_to_say_leaves_the_stream_silent() {
    let live = Live::opening(Stopped::at(0).as_ref());
    let mut listening = live.listening(None).await;

    live.refresh(&Mute).await;

    assert_eq!(
        listening.next().await.as_deref(),
        Some(BEAT_SAID),
        "a beat, not something nothing gathered"
    );
}

#[tokio::test(start_paused = true)]
async fn the_gather_comes_round_again_on_the_tick() {
    let live = Arc::new(Live::opening(Stopped::at(0).as_ref()));
    let mut listening = live.listening(None).await;
    let gathering =
        tokio::spawn(Arc::clone(&live).gathering(Arc::new(Counting(AtomicUsize::new(0)))));

    let began = Instant::now();
    let first = listening.next().await;
    let straight_away = began.elapsed();
    let again = listening.next().await;
    let round_again = began.elapsed();
    gathering.abort();

    assert!(first.is_some_and(|said| said.contains(r#""data":0"#)));
    assert!(again.is_some_and(|said| said.contains(r#""data":1"#)));
    assert_eq!(straight_away, Duration::ZERO);
    assert_eq!(round_again, TICK);
}

#[tokio::test(start_paused = true)]
async fn a_listener_arriving_is_gathered_for_rather_than_made_to_wait() {
    let live = Arc::new(Live::opening(Stopped::at(0).as_ref()));
    let mut listening = live.listening(None).await;
    live.nudge();
    let gathering =
        tokio::spawn(Arc::clone(&live).gathering(Arc::new(Counting(AtomicUsize::new(0)))));

    let began = Instant::now();
    listening.next().await;
    listening.next().await;
    let asked_for = began.elapsed();
    gathering.abort();

    assert_eq!(
        asked_for,
        Duration::ZERO,
        "what it holds is stale until a gather made since replaces it"
    );
}

#[tokio::test]
async fn the_stream_is_fed_by_the_gather_that_answers_the_dashboard() {
    let live = Live::opening(Stopped::at(0).as_ref());
    let mut listening = live.listening(None).await;
    let dashboard = Dashboard::against(Arc::new(nowhere()));

    live.refresh(&dashboard).await;
    // Twice, because the second gather is handed the first to carry a figure a
    // silent source has stopped giving forward from.
    live.refresh(&dashboard).await;

    let heard = listening.next().await.unwrap_or_default();
    assert!(heard.contains("\nevent: dashboard\n"), "{heard}");
    assert!(heard.contains(r#""kind":"dashboard""#), "{heard}");
    assert!(heard.contains(r#""telemetry":"#), "{heard}");
    assert!(heard.contains(r#""health":"#), "{heard}");
    assert!(listening.next().await.is_some(), "and the one after it");
}
