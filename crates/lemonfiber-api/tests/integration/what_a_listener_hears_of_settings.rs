//! The settings, heard on the stream.
//!
//! Their read's envelope, said to the operator as they arrive and whenever a setting
//! changes, and to nobody else: a key listening to the same stream passes over it, and a
//! member's own stream never carries it.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderName, HeaderValue, Response};
use axum::Extension;
use futures_util::StreamExt;
use lemonfiber_api::admission::{Admitting, Caller, Keyed, Keyring};
use lemonfiber_api::events::extending::Together;
use lemonfiber_api::events::live::Live;
use lemonfiber_api::events::settings::Settings;
use lemonfiber_api::events::wire::{Audience, Nature, Rendered};
use lemonfiber_api::events::{stream, Streaming};
use lemonfiber_api::guard::{Arrived, Binding, Token, TOKEN_HEADER};
use lemonfiber_core::app::{Command, Settling};
use lemonfiber_core::keys::Scope;
use lemonfiber_core::model::{kind, Envelope};
use lemonfiber_fixtures::ports::{Chance, Stopped};

use crate::reading::{as_the_command_renders_it, configured};
use crate::what_a_key_admits::{keeping_keys, HERE};

/// The name an event goes by on the wire.
fn named(said: &str) -> Option<&str> {
    said.lines().find_map(|line| line.strip_prefix("event: "))
}

/// The envelope an event carries.
fn carried(said: &str) -> String {
    said.lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whether nothing more is heard within a moment.
async fn silent(listening: &mut lemonfiber_api::events::live::Listening) -> bool {
    tokio::time::timeout(std::time::Duration::from_millis(50), listening.next())
        .await
        .is_err()
}

#[tokio::test]
async fn the_operator_arriving_hears_the_settings_as_the_read_answers_them() {
    let ctx = configured("settings-stream-arriving", "");
    let live = Live::opening(Stopped::at(0).as_ref());
    let mut listening = live.listening(None).await.heard_as(Audience::Operator);
    live.refresh(&Settings::against(Arc::new(ctx.clone())))
        .await;

    let said = listening.next().await.unwrap_or_default();
    assert_eq!(named(&said), Some("settings"), "{said}");
    assert_eq!(
        Some(carried(&said)),
        as_the_command_renders_it(&ctx, Command::Settings(Settling::Read)).await
    );
}

#[tokio::test]
async fn the_settings_are_said_again_only_when_one_changes() {
    let ctx = configured("settings-stream-changed", "");
    let live = Live::opening(Stopped::at(0).as_ref());
    let settings = Settings::against(Arc::new(ctx.clone()));
    let mut listening = live.listening(None).await.heard_as(Audience::Operator);
    live.refresh(&settings).await;
    live.refresh(&settings).await;

    let first = listening.next().await.unwrap_or_default();
    assert_eq!(named(&first), Some("settings"), "{first}");
    assert!(
        silent(&mut listening).await,
        "an unchanged tick said something"
    );

    if let Some(env) = ctx.settings.env_file.as_deref() {
        assert!(std::fs::write(env, "VPN_COUNTRIES=Netherlands\n").is_ok());
    }
    live.refresh(&settings).await;
    let changed = listening.next().await.unwrap_or_default();
    assert_eq!(named(&changed), Some("settings"), "{changed}");
    assert!(changed.contains("Netherlands"), "{changed}");
}

#[tokio::test]
async fn a_listener_hearing_as_anyone_never_hears_the_settings() {
    let ctx = Arc::new(configured("settings-stream-anyone", ""));
    let live = Live::opening(Stopped::at(0).as_ref());
    let mut operator = live.listening(None).await.heard_as(Audience::Operator);
    let mut anyone = live.listening(None).await;
    live.refresh(&Together::in_a_run(&ctx)).await;

    let mut heard = Vec::new();
    while let Ok(Some(said)) =
        tokio::time::timeout(std::time::Duration::from_millis(50), anyone.next()).await
    {
        heard.push(named(&said).map(str::to_owned));
    }
    assert_eq!(
        heard,
        ["dashboard", "news", "playing", "plugins", "wiring"].map(|name| Some(name.to_owned()))
    );
    let mut told = Vec::new();
    for _ in 0..6 {
        let said = operator.next().await.unwrap_or_default();
        told.push(named(&said).map(str::to_owned));
    }
    assert_eq!(told.last(), Some(&Some("settings".to_owned())), "{told:?}");
}

#[test]
fn only_the_per_run_token_and_an_operator_session_hear_as_the_operator() {
    let key = |scope: Scope| {
        Caller::Key(Keyed {
            name: "a program".to_owned(),
            scope,
        })
    };
    assert_eq!(Audience::of(&Caller::Machine), Audience::Operator);
    assert_eq!(Audience::of(&Caller::Operator), Audience::Operator);
    for caller in [
        Caller::Member("a7f3".to_owned()),
        key(Scope::Read),
        key(Scope::Act),
        key(Scope::Member {
            id: "a7f3".to_owned(),
            name: "ana".to_owned(),
        }),
    ] {
        assert_eq!(Audience::of(&caller), Audience::Anyone, "{caller:?}");
    }
    assert!(!Audience::Anyone.hears(Audience::Operator));
    assert!(Audience::Anyone.hears(Audience::Anyone));
    assert!(Audience::Operator.hears(Audience::Anyone));
}

/// A stream opened over the route with `secret`, and the gather it listens to.
async fn opened(
    secret: &str,
    admitting: Arc<Admitting>,
    token: Token,
) -> (Response<Body>, Arc<Live>) {
    let live = Arc::new(Live::opening(Stopped::at(0).as_ref()));
    let streaming = Arc::new(Streaming {
        admitting,
        token: Arc::new(token),
        bound: Binding::here(8471),
        live: Arc::clone(&live),
        clock: Stopped::at(0),
        reading: Arc::new(crate::idle::ctx()),
    });
    let mut headers = HeaderMap::new();
    for (name, value) in [(TOKEN_HEADER, secret), ("host", "127.0.0.1:8471")] {
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(name.to_lowercase().as_bytes()),
            HeaderValue::from_str(value),
        ) {
            headers.insert(name, value);
        }
    }
    let arrived = Extension(Arrived {
        from: HERE,
        encrypted: false,
    });
    (stream(State(streaming), Some(arrived), headers).await, live)
}

/// What is said to the operator alone, then what is said to anyone.
async fn both_said(live: &Live) {
    let settings = Rendered::of(Nature::State, &Envelope::new(kind::SETTINGS, "held"))
        .map(Rendered::to_the_operator);
    live.say_if_rendered(settings).await;
    live.say_if_rendered(Rendered::of(
        Nature::State,
        &Envelope::new(kind::NEWS, "anyone"),
    ))
    .await;
}

/// The first thing that comes down an answered stream.
async fn first(answer: Response<Body>) -> String {
    answer
        .into_body()
        .into_data_stream()
        .next()
        .await
        .and_then(Result::ok)
        .map(|said| String::from_utf8_lossy(&said).into_owned())
        .unwrap_or_default()
}

#[tokio::test]
async fn a_key_on_the_stream_passes_over_what_is_said_to_the_operator() {
    let keys = keeping_keys("settings-stream");
    let admitting = Arc::new(Admitting {
        keys: Keyring::at(Some(keys.kept.clone()), Some(keys.used.clone())),
        ..Admitting::default()
    });
    for secret in [&keys.read, &keys.act] {
        let Some(token) = Token::mint(&Chance::cycling()) else {
            unreachable!("cycling letters always supply bytes");
        };
        let (answer, live) = opened(secret.as_str(), Arc::clone(&admitting), token).await;
        both_said(&live).await;
        let said = first(answer).await;
        assert_eq!(named(&said), Some("news"), "{said}");
    }
}

#[tokio::test]
async fn the_per_run_token_on_the_stream_hears_what_is_said_to_the_operator() {
    let Some(token) = Token::mint(&Chance::cycling()) else {
        unreachable!("cycling letters always supply bytes");
    };
    let secret = token.as_str().to_owned();
    let (answer, live) = opened(&secret, Arc::new(Admitting::default()), token).await;
    both_said(&live).await;
    let said = first(answer).await;
    assert_eq!(named(&said), Some("settings"), "{said}");
}

#[tokio::test]
async fn a_listener_coming_back_is_not_handed_what_was_said_to_the_operator_alone() {
    let live = Live::opening(Stopped::at(0).as_ref());
    let record = |what: &'static str| Rendered::of(Nature::Record, &Envelope::new(kind::LOG, what));
    let Some(before) = record("before") else {
        unreachable!("a log line renders");
    };
    let seen = live.say(before).await;
    live.say_if_rendered(record("theirs").map(Rendered::to_the_operator))
        .await;
    live.say_if_rendered(record("anyone's")).await;

    let mut anyone = live.listening(Some(seen.id())).await;
    let mut operator = live
        .listening(Some(seen.id()))
        .await
        .heard_as(Audience::Operator);
    let missed = anyone.next().await.unwrap_or_default();
    assert!(missed.contains("anyone's"), "{missed}");
    let told = operator.next().await.unwrap_or_default();
    assert!(told.contains("theirs"), "{told}");
}
