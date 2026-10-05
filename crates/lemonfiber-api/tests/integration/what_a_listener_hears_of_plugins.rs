//! What is installed and what the stack wires to what, heard on the stream.
//!
//! Each is its read's envelope, said to a listener as it arrives and to everyone when
//! it changes, and not on every tick: a phone redraws its plugin screen and its wiring
//! from these, and an unchanged envelope is nothing to wake it for.

use std::sync::Arc;

use lemonfiber_api::events::extending::Extending;
use lemonfiber_api::events::live::Live;
use lemonfiber_core::app::plugins::Asked as Installing;
use lemonfiber_core::app::{Command, Linking};
use lemonfiber_fixtures::ports::Stopped;

use crate::reading::{as_the_command_renders_it, configured};

/// A world whose record of what is installed holds `record`.
fn recording(named: &str, record: &str) -> lemonfiber_core::app::Ctx {
    let ctx = configured(named, "LEMONFIBER_USENET=on\n");
    if let Some(env) = ctx.settings.env_file.as_deref() {
        let _ = std::fs::write(env.with_file_name("plugins.json"), record);
    }
    ctx
}

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

#[tokio::test]
async fn a_listener_arriving_hears_what_is_installed_and_what_is_wired() {
    let ctx = recording("stream-arriving", "");
    let live = Live::opening(Stopped::at(0).as_ref());
    let mut listening = live.listening(None).await;
    live.refresh(&Extending::against(Arc::new(ctx.clone())))
        .await;

    let plugins = listening.next().await.unwrap_or_default();
    let wiring = listening.next().await.unwrap_or_default();
    assert_eq!(named(&plugins), Some("plugins"), "{plugins}");
    assert_eq!(named(&wiring), Some("wiring"), "{wiring}");
    assert_eq!(
        Some(carried(&plugins)),
        as_the_command_renders_it(&ctx, Command::Plugins(Installing::Installed)).await,
        "the envelope the read answers with"
    );
    assert_eq!(
        Some(carried(&wiring)),
        as_the_command_renders_it(&ctx, Command::Wiring(Linking::Read)).await
    );
}

#[tokio::test]
async fn neither_is_said_again_until_it_changes() {
    let ctx = recording("stream-unchanged", "");
    let live = Live::opening(Stopped::at(0).as_ref());
    let extending = Extending::against(Arc::new(ctx.clone()));
    let mut listening = live.listening(None).await;
    live.refresh(&extending).await;
    live.refresh(&extending).await;

    let first = listening.next().await.unwrap_or_default();
    let second = listening.next().await.unwrap_or_default();
    assert_eq!(
        (named(&first), named(&second)),
        (Some("plugins"), Some("wiring"))
    );
    let silence = tokio::time::timeout(std::time::Duration::from_millis(50), listening.next())
        .await
        .is_err();
    assert!(silence, "an unchanged tick said something");

    // The record changes: something is installed.
    let record = r#"{"installed":[{"plugin":"komga","version":"1.2.0","services":[],"provides":[],"contributions":[],"from":"./komga","revision":"","signed":"","installed_at":"1700000000"}]}"#;
    if let Some(env) = ctx.settings.env_file.as_deref() {
        assert!(std::fs::write(env.with_file_name("plugins.json"), record).is_ok());
    }
    live.refresh(&extending).await;
    let changed = listening.next().await.unwrap_or_default();
    assert_eq!(named(&changed), Some("plugins"), "{changed}");
    assert!(changed.contains(r#""plugin":"komga""#), "{changed}");
}

#[tokio::test]
async fn a_record_that_will_not_read_is_not_said_as_nothing_installed() {
    let ctx = recording("stream-damaged", "{ not json");
    let live = Live::opening(Stopped::at(0).as_ref());
    let mut listening = live.listening(None).await;
    live.refresh(&Extending::against(Arc::new(ctx))).await;

    let silence = tokio::time::timeout(std::time::Duration::from_millis(50), listening.next())
        .await
        .is_err();
    assert!(silence, "a damaged record was said as something");
}

/// The stream has one gather, and what it says from several sources is said from one
/// that asks each in turn: the dashboard first, then the plugins and the wiring.
#[tokio::test]
async fn several_sources_are_gathered_as_one_in_the_order_given() {
    use lemonfiber_api::events::extending::Together;

    let ctx = Arc::new(recording("stream-together", ""));
    let live = Live::opening(Stopped::at(0).as_ref());
    let mut listening = live.listening(None).await;
    let together = Together::in_a_run(&ctx);
    live.refresh(&together).await;

    let mut names = Vec::new();
    for _ in 0..4 {
        let said = listening.next().await.unwrap_or_default();
        names.push(named(&said).map(str::to_owned));
    }
    assert_eq!(
        names,
        ["dashboard", "news", "plugins", "wiring"].map(|name| Some(name.to_owned()))
    );
}
