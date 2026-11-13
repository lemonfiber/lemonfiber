//! What an action sent again under its key is answered with, and what it runs.
//!
//! The load-bearing property is that the action runs once. A client that heard
//! nothing back sends the same request again under the same key, and the stack must
//! answer it with what the first send was answered with rather than take the
//! machine's services down a second time. So these count what reached the machine,
//! not only what came back.
//!
//! Driven from outside the crate, because what a caller can reach is the thing
//! worth holding still.

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::to_bytes;
use axum::http::header;
use axum::Extension;
use lemonfiber_api::actions;
use lemonfiber_api::actions::again::HEADER;
use lemonfiber_api::admission::Caller;
use lemonfiber_api::events::live::Live;
use lemonfiber_api::guard::Token;
use lemonfiber_api::jobs::Jobs;
use lemonfiber_api::router::Serving;
use lemonfiber_core::app::Ctx;
use lemonfiber_core::config::{
    resending::{DEFAULT_KEYS, DEFAULT_MINUTES},
    Settings, IDEMPOTENCY_KEYS_KEY, IDEMPOTENCY_MINUTES_KEY,
};
use lemonfiber_core::ports::hosting::{Host, Manager};
use lemonfiber_fixtures::hosting::Hosting;
use lemonfiber_fixtures::ports::{Chance, Following, Idle, Stopped};
use lemonfiber_fixtures::pulled::Pulled;

use crate::idle::ctx;

/// What every request in one test is served from, so a second send meets the first.
fn serving(ctx: Ctx) -> Serving {
    let Some(token) = Token::mint(&Chance::cycling()) else {
        unreachable!("cycling letters always supply bytes");
    };
    Serving::for_run(
        Arc::new(ctx),
        Arc::new(token),
        lemonfiber_api::guard::Binding::here(8471),
        Jobs::default(),
        Arc::new(lemonfiber_api::admission::Admitting::default()),
        Arc::new(Live::opening(Stopped::at(0).as_ref())),
    )
}

/// A world whose service manager is the one given.
fn managed(host: &Arc<Hosting>) -> Serving {
    serving(ctx().with_hosting(Arc::clone(host) as Arc<dyn Host>))
}

/// The idle world over `host`, on a clock that moves with the runtime's own time,
/// keeping its settings at `settings` where a test names a file.
fn following(host: &Arc<Hosting>, settings: Option<PathBuf>) -> Serving {
    let world = lemonfiber_testing::a_live_context()
        .runner(Arc::new(Idle))
        .images(Pulled::holding(Vec::new()))
        .over(lemonfiber_testing::nowhere())
        .clock(Following::started())
        .settings(Settings {
            env_file: settings,
            ..Settings::default()
        })
        .build()
        .with_hosting(Arc::clone(host) as Arc<dyn Host>);
    serving(world)
}

/// A settings file of this test's own, holding `contents`.
fn settings_holding(name: &str, contents: &str) -> PathBuf {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("again-{name}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join(".env");
    let written = std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&path, contents));
    assert!(written.is_ok(), "the scratch settings file is written");
    path
}

/// The action routes over `serving`, asked by `who`.
///
/// The subject is mounted here because these routes are built without the guard that
/// puts it on a request in a run.
fn routed(serving: &Serving, who: Caller) -> axum::Router {
    actions::routes()
        .with_state(serving.clone())
        .layer(Extension(who))
}

/// What one request answered, as its status and what it said.
async fn sent(router: axum::Router, action: &str, body: &str, keys: &[&str]) -> (u16, String) {
    let mut request = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/actions/{action}"))
        .header(header::CONTENT_TYPE, "application/json");
    for key in keys {
        request = request.header(HEADER, *key);
    }
    let Ok(request) = request.body(axum::body::Body::from(body.to_owned())) else {
        unreachable!("a request built from values that are already headers cannot fail");
    };
    let Some(response) = tower::ServiceExt::oneshot(router, request).await.ok() else {
        unreachable!("the router is infallible; its handlers answer rather than fail");
    };
    let status = response.status().as_u16();
    let read = to_bytes(response.into_body(), usize::MAX).await;
    let bytes = read.map(|bytes| bytes.to_vec()).unwrap_or_default();
    (status, String::from_utf8(bytes).unwrap_or_default())
}

/// The least a clock is moved by.
const SECOND: std::time::Duration = std::time::Duration::from_secs(1);

/// Taking the request clock back off the machine.
const EXPIRING: &str = r#"{"kept":"expiring"}"#;

/// What one machine-run send of that answered.
async fn withdrawing(serving: &Serving, keys: &[&str]) -> (u16, String) {
    sent(
        routed(serving, Caller::Machine),
        "hosting-remove",
        EXPIRING,
        keys,
    )
    .await
}

#[tokio::test]
async fn an_action_sent_again_under_its_key_runs_once_and_is_answered_the_same() {
    let host = Hosting::with(Manager::Systemd);
    let serving = managed(&host);
    let first = withdrawing(&serving, &["attempt-1"]).await;
    let again = withdrawing(&serving, &["attempt-1"]).await;
    assert_eq!(first.0, 200, "{}", first.1);
    assert_eq!(again, first);
    assert_eq!(host.withdrawn().len(), 1);
}

#[tokio::test]
async fn an_action_sent_again_without_a_key_runs_again() {
    let host = Hosting::with(Manager::Systemd);
    let serving = managed(&host);
    withdrawing(&serving, &[]).await;
    withdrawing(&serving, &[]).await;
    assert_eq!(host.withdrawn().len(), 2);
}

#[tokio::test]
async fn a_second_send_arriving_while_the_first_runs_waits_for_it_and_runs_nothing() {
    let host = Hosting::shut(Manager::Systemd);
    let serving = managed(&host);
    let first = tokio::spawn({
        let serving = serving.clone();
        async move { withdrawing(&serving, &["attempt-1"]).await }
    });
    host.begun().await;
    let again = tokio::spawn({
        let serving = serving.clone();
        async move { withdrawing(&serving, &["attempt-1"]).await }
    });
    for _ in 0..8 {
        tokio::task::yield_now().await;
    }
    assert!(!again.is_finished());
    host.open();
    let (first, again) = (first.await.ok(), again.await.ok());
    assert!(first.as_ref().is_some_and(|(status, _)| *status == 200));
    assert_eq!(again, first);
    assert_eq!(host.withdrawn().len(), 1);
}

#[tokio::test]
async fn a_key_sent_again_with_other_arguments_or_another_action_is_refused_and_runs_nothing() {
    let host = Hosting::with(Manager::Systemd);
    let serving = managed(&host);
    withdrawing(&serving, &["attempt-1"]).await;
    let watching = ("hosting-remove", r#"{"kept":"watch"}"#);
    let installing = ("hosting-install", EXPIRING);
    for (action, body) in [watching, installing] {
        let routes = routed(&serving, Caller::Machine);
        let (status, said) = sent(routes, action, body, &["attempt-1"]).await;
        assert_eq!(status, 400, "{action}");
        assert!(said.contains(r#""code":"ASK-13""#), "{said}");
    }
    assert_eq!(host.withdrawn().len(), 1);
    assert!(host.placed().is_empty());
}

#[tokio::test]
async fn a_key_that_cannot_be_read_as_one_is_refused_before_anything_runs() {
    let host = Hosting::with(Manager::Systemd);
    let serving = managed(&host);
    let longest = "k".repeat(lemonfiber_api::actions::again::LONGEST + 1);
    for keys in [
        vec!["with space"],
        vec!["one", "two"],
        vec![longest.as_str()],
    ] {
        let (status, said) = withdrawing(&serving, &keys).await;
        assert_eq!(status, 400, "{keys:?}");
        assert!(said.contains(r#""code":"ASK-12""#), "{said}");
    }
    assert_eq!(host.withdrawn().len(), 0);
}

#[tokio::test]
async fn one_callers_key_is_not_another_callers_attempt() {
    let host = Hosting::with(Manager::Systemd);
    let serving = managed(&host);
    withdrawing(&serving, &["attempt-1"]).await;
    let operator = routed(&serving, Caller::Operator);
    let (status, _) = sent(operator, "hosting-remove", EXPIRING, &["attempt-1"]).await;
    assert_eq!(status, 200);
    assert_eq!(host.withdrawn().len(), 2);
}

#[tokio::test]
async fn work_sent_again_under_its_key_is_answered_with_the_same_name() {
    let serving = serving(ctx().with_random(Arc::new(Chance::turning(0))));
    let restart = |keys: &'static [&'static str]| {
        sent(
            routed(&serving, Caller::Machine),
            "restart",
            r#"{"forms":["tv"]}"#,
            keys,
        )
    };
    let (first, again) = (restart(&["attempt-1"]).await, restart(&["attempt-1"]).await);
    assert_eq!(first.0, 202, "{}", first.1);
    assert_eq!(again, first);
    let (one, other) = (restart(&[]).await, restart(&[]).await);
    assert_ne!(one, other);
}

#[tokio::test(start_paused = true)]
async fn an_attempt_is_forgotten_once_it_has_been_remembered_for_half_an_hour() {
    let host = Hosting::with(Manager::Systemd);
    let serving = following(&host, None);
    let within = std::time::Duration::from_secs(DEFAULT_MINUTES * 60);
    withdrawing(&serving, &["attempt-1"]).await;
    tokio::time::advance(within.saturating_sub(SECOND)).await;
    withdrawing(&serving, &["attempt-1"]).await;
    assert_eq!(host.withdrawn().len(), 1);
    tokio::time::advance(SECOND).await;
    withdrawing(&serving, &["attempt-1"]).await;
    assert_eq!(host.withdrawn().len(), 2);
}

#[tokio::test(start_paused = true)]
async fn how_long_and_how_many_are_what_the_settings_say_at_each_send() {
    let host = Hosting::with(Manager::Systemd);
    let path = settings_holding(
        "configured",
        &format!("{IDEMPOTENCY_MINUTES_KEY}=2\n{IDEMPOTENCY_KEYS_KEY}=2\n"),
    );
    let serving = following(&host, Some(path));
    for one in ["k0", "k1", "k2"] {
        withdrawing(&serving, &[one]).await;
    }
    withdrawing(&serving, &["k1"]).await;
    assert_eq!(host.withdrawn().len(), 3);
    withdrawing(&serving, &["k0"]).await;
    assert_eq!(host.withdrawn().len(), 4);
    tokio::time::advance(std::time::Duration::from_secs(2 * 60)).await;
    withdrawing(&serving, &["k0"]).await;
    assert_eq!(host.withdrawn().len(), 5);
}

#[tokio::test]
async fn past_the_most_one_caller_has_remembered_their_oldest_attempt_runs_again() {
    let host = Hosting::with(Manager::Systemd);
    let serving = managed(&host);
    let at_most = DEFAULT_KEYS;
    for one in 0..=at_most {
        withdrawing(&serving, &[&format!("k{one}")]).await;
    }
    assert_eq!(host.withdrawn().len(), at_most + 1);
    withdrawing(&serving, &["k2"]).await;
    assert_eq!(host.withdrawn().len(), at_most + 1);
    withdrawing(&serving, &["k0"]).await;
    assert_eq!(host.withdrawn().len(), at_most + 2);
}

#[tokio::test]
async fn work_that_falls_over_answers_both_sends_with_a_failure_and_its_key_is_forgotten() {
    let host = Hosting::shut(Manager::Systemd);
    host.fall_over();
    let serving = managed(&host);
    let first = tokio::spawn({
        let serving = serving.clone();
        async move { withdrawing(&serving, &["attempt-1"]).await }
    });
    host.begun().await;
    let again = tokio::spawn({
        let serving = serving.clone();
        async move { withdrawing(&serving, &["attempt-1"]).await }
    });
    for _ in 0..8 {
        tokio::task::yield_now().await;
    }
    host.open();
    let (first, again) = (first.await.ok(), again.await.ok());
    let Some((status, said)) = first.clone() else {
        unreachable!("the first send is answered");
    };
    assert_eq!(status, 500);
    assert!(said.contains(r#""code":"SERVE-8""#), "{said}");
    assert_eq!(again, first);
    assert_eq!(host.withdrawn().len(), 0);
    let (status, _) = withdrawing(&serving, &["attempt-1"]).await;
    assert_eq!(status, 200);
    assert_eq!(host.withdrawn().len(), 1);
}
