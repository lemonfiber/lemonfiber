//! What a browser gets when it asks to keep reading, and where the lines go.
//!
//! The load-bearing property is that following is the same request as reading the
//! scrollback and not a second one. It is asked for at the same endpoint, with the
//! flag the command line spells it with, and what differs is only that it cannot be
//! answered with what it read — so it is answered with a name for the work, and the
//! lines arrive on the stream the browser is already holding open.
//!
//! Driven from outside the crate, because what a caller can reach is the thing
//! worth holding still.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use axum::body::to_bytes;
use axum::http::StatusCode;
use lemonfiber_api::events::live::Live;
use lemonfiber_api::guard::{Token, TOKEN_HEADER};
use lemonfiber_api::jobs::{Jobs, Standing};
use lemonfiber_api::router::Serving;
use lemonfiber_core::app::Ctx;
use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::docker::{Health, Lifecycle};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::Fake;
use lemonfiber_fixtures::ports::{Chance, Idle, Stopped};
use lemonfiber_fixtures::support::Reporting;

/// Bytes the test chose, so a token is the same one twice.
fn given() -> Chance {
    Chance::cycling()
}

/// The stack this repository carries, read from disk.
fn stack() -> Source {
    Source::External(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/media-stack"
    )))
}

/// The world a follow runs against: what the engine says, and nothing else.
fn world(engine: Reporting, stack: Source) -> Ctx {
    Ctx::new(
        Arc::new(Idle),
        Arc::new(engine),
        Arc::new(lemonfiber_core::adapters::System),
        Arc::new(lemonfiber_core::adapters::Disk),
        stack,
        Settings::default(),
        Environment::MacOs,
    )
    .with_http(Fake::silent())
}

/// An engine holding one service that has said two things.
fn talking() -> Reporting {
    Reporting::holding(&["sonarr"], Lifecycle::Running, Health::Healthy)
        .saying_at("sonarr", "2026-01-01T00:00:00Z", "started")
        .saying_at("sonarr", "2026-01-01T00:00:01Z", "importing")
}

/// One run of the surface: what it serves from, and the stream it says on.
struct Run {
    serving: Serving,
    live: Arc<Live>,
}

impl Run {
    /// A run over this world, or nothing where the machine gave no token.
    ///
    /// Both answers are asked for below, so neither is a line nothing runs.
    fn over(ctx: Ctx) -> Option<Self> {
        // At the epoch, so the run names its first event `0-1` and a listener can
        // ask for everything since `0-0` without guessing what the run calls itself.
        let live = Arc::new(Live::opening(Stopped::at(0).as_ref()));
        Some(Self {
            serving: Serving {
                ctx: Arc::new(ctx),
                token: Arc::new(Token::mint(&given())?),
                bound: ([127, 0, 0, 1], 8471).into(),
                admitting: Arc::new(lemonfiber_api::admission::Admitting::default()),
                jobs: Jobs::default(),
                live: Arc::clone(&live),
            },
            live,
        })
    }

    /// What a request to this path is answered with.
    async fn asked(&self, path: &str) -> Option<(StatusCode, String)> {
        let request = axum::http::Request::builder()
            .uri(path)
            .header("host", "127.0.0.1:8471")
            .header(TOKEN_HEADER, self.serving.token.as_str())
            .body(axum::body::Body::empty())
            .ok()?;
        let router = lemonfiber_api::read::routes().with_state(self.serving.clone());
        let response = tower::ServiceExt::oneshot(router, request).await.ok()?;
        let status = response.status();
        let read = to_bytes(response.into_body(), usize::MAX).await.ok()?;
        Some((status, String::from_utf8(read.to_vec()).ok()?))
    }

    /// The name a follow was answered with, read out of the envelope.
    async fn followed(&self, path: &str) -> Option<String> {
        let (status, body) = self.asked(path).await?;
        if status != StatusCode::ACCEPTED {
            return None;
        }
        let parsed: serde_json::Value = serde_json::from_str(&body).ok()?;
        parsed.get("data")?.get("job")?.as_str().map(str::to_owned)
    }

    /// Where a name got to, once the work has had the chance to get anywhere.
    async fn settled(&self, job: &str) -> Option<Standing> {
        for _ in 0..200 {
            match self.serving.jobs.about(job).await.map(|work| work.standing) {
                Some(Standing::Running) | None => {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
                settled => return settled,
            }
        }
        self.serving.jobs.about(job).await.map(|work| work.standing)
    }

    /// Everything the stream said, as a client that missed all of it reads it.
    async fn heard(&self) -> String {
        let mut listening = self.live.listening(Some("0-0")).await;
        let mut said = String::new();
        while let Some(event) = listening.next().await {
            // The beat is what a stream says when it has nothing left to say, so
            // it is where the record this test is reading ends.
            if event.starts_with(':') {
                break;
            }
            said.push_str(&event);
        }
        said
    }
}

// ── The request, and what it is answered with ────────────────────────────────

#[tokio::test]
async fn asking_to_keep_reading_is_answered_with_a_name_rather_than_with_lines() {
    // The scrollback ends and is answered with what it read. A follow does not
    // end, so there is nothing to answer with — and the answer is the same one
    // every request that outlives its own connection gets here.
    let Some(run) = Run::over(world(talking(), stack())) else {
        unreachable!("cycling letters always supply bytes");
    };
    let seen = run.asked("/api/logs?service=sonarr&follow=true").await;
    let Some((status, body)) = seen else {
        unreachable!("the router answers rather than fails");
    };
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    assert!(body.contains(r#""kind":"job""#), "{body}");
    assert!(
        body.contains(r#""action":"logs""#),
        "the request it answers"
    );
}

#[tokio::test]
async fn not_asking_to_keep_reading_is_the_scrollback_it_always_was() {
    let Some(run) = Run::over(world(talking(), stack())) else {
        unreachable!("cycling letters always supply bytes");
    };
    for path in [
        "/api/logs?service=sonarr",
        "/api/logs?service=sonarr&follow=false",
    ] {
        let seen = run.asked(path).await;
        assert!(
            seen.is_some_and(
                |(status, body)| status == StatusCode::OK && body.contains(r#""kind":"log""#)
            ),
            "{path}"
        );
    }
}

#[tokio::test]
async fn a_follow_that_is_neither_yes_nor_no_is_refused_rather_than_read_as_either() {
    // The two answers are different shapes — lines, or a name — so a caller that
    // meant to follow would otherwise parse an answer it never asked for.
    let Some(run) = Run::over(world(talking(), stack())) else {
        unreachable!("cycling letters always supply bytes");
    };
    assert_eq!(
        run.asked("/api/logs?follow=maybe").await,
        Some((
            StatusCode::BAD_REQUEST,
            "Whether to keep reading must be true or false.".to_owned()
        ))
    );
}

// ── Where the lines go ───────────────────────────────────────────────────────

#[tokio::test]
async fn the_lines_a_follow_reads_arrive_on_the_stream_the_browser_already_holds() {
    // The whole point of answering with a name: what the service says afterwards
    // has somewhere to arrive, and it arrives under the kind every other envelope's
    // event name comes from — so a browser that is not following never sees one.
    let Some(run) = Run::over(world(talking(), stack())) else {
        unreachable!("cycling letters always supply bytes");
    };
    let Some(job) = run.followed("/api/logs?service=sonarr&follow=true").await else {
        unreachable!("a follow is answered with a name");
    };
    assert_eq!(run.settled(&job).await, Some(Standing::Ended));

    let said = run.heard().await;
    assert!(said.contains("event: log"), "{said}");
    assert!(said.contains(r#""line":"started""#), "{said}");
    assert!(said.contains(r#""line":"importing""#), "in order: {said}");
}

#[tokio::test]
async fn a_follow_takes_the_same_narrowing_the_scrollback_takes() {
    // Every flag the request accepts: which form, which service, and how many lines
    // to begin with. A follow that dropped one would be answering a request nobody
    // made, and the dropping is invisible — the name comes back either way.
    let engine = talking().saying_at("jellyfin", "2026-01-01T00:00:02Z", "listening");
    let Some(run) = Run::over(world(engine, stack())) else {
        unreachable!("cycling letters always supply bytes");
    };
    let Some(job) = run
        .followed("/api/logs?form=library&tail=1&follow=true")
        .await
    else {
        unreachable!("a follow is answered with a name");
    };
    assert_eq!(run.settled(&job).await, Some(Standing::Ended));

    let said = run.heard().await;
    assert!(said.contains(r#""service":"jellyfin""#), "{said}");
    assert!(
        !said.contains(r#""service":"sonarr""#),
        "a form is the services it declares, and sonarr is not one: {said}"
    );
}

#[tokio::test]
async fn a_follow_ends_when_there_is_nothing_left_to_read_rather_than_with_an_outcome() {
    // It ends because it was released, because the run ended, or because the
    // containers stopped having anything to say. None of those is a value, so
    // there is no outcome to redeem the name for — only that it is over.
    let Some(run) = Run::over(world(talking(), stack())) else {
        unreachable!("cycling letters always supply bytes");
    };
    let Some(job) = run.followed("/api/logs?follow=true").await else {
        unreachable!("a follow is answered with a name");
    };
    assert_eq!(run.settled(&job).await, Some(Standing::Ended));
}

#[tokio::test]
async fn a_stack_that_cannot_be_read_stops_the_follow_under_its_own_name() {
    // The stream is opened inside the work rather than before it, so a stack that
    // cannot be read is reported the way every other failure under a name is —
    // instead of being a second shape of answer to the same request.
    let Some(run) = Run::over(world(
        talking(),
        Source::External(Path::new("/lemonfiber/no/such/stack")),
    )) else {
        unreachable!("cycling letters always supply bytes");
    };
    let Some(job) = run.followed("/api/logs?follow=true").await else {
        unreachable!("a follow is answered with a name");
    };
    assert!(
        matches!(run.settled(&job).await, Some(Standing::Failed(said, _))
            if said.contains(r#""kind":"error""#)),
        "the failure is the envelope every other one arrives in"
    );
}

#[tokio::test]
async fn work_that_cannot_be_named_is_not_begun() {
    // A follow with no name is work nothing could ever stop, so it is refused
    // rather than begun and left running.
    let ctx = world(talking(), stack()).with_random(Arc::new(Chance::exactly(None)));
    let Some(run) = Run::over(ctx) else {
        unreachable!("cycling letters always supply bytes");
    };
    let seen = run.asked("/api/logs?follow=true").await;
    assert!(
        seen.is_some_and(|(status, body)| status == StatusCode::INTERNAL_SERVER_ERROR
            && body.contains("randomness")),
        "a name it cannot mint stops it"
    );
}
