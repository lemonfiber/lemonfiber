//! An event is taken only with the key its service was handed, and told as it happened.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::ports::Chance;

use super::{intake, minted, told, Event, Happened};
use crate::alert::Notification;
use crate::app::Ctx;
use crate::config::Settings;
use crate::error::codes::notify::EVENTS_REFUSED;
use crate::error::Severity;
use crate::model::BackendKind;
use crate::notify::backends;
use crate::notify::configuring::{asked, Adding, Asked};
use crate::ports::http::Http;
use crate::ports::random::Random;

/// Randomness that differs on every ask, so two keys minted one after the other differ.
struct Turning(AtomicU8);

impl Random for Turning {
    fn bytes(&self, n: usize) -> Option<Vec<u8>> {
        Some(vec![self.0.fetch_add(1, Ordering::SeqCst); n])
    }
}

fn machine(name: &str) -> Ctx {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(dir.join("config"));
    crate::test_support::a_context()
        .settings(Settings {
            env_file: Some(dir.join("config").join(".env")),
            stack_dir: Some(dir.join("data").join("stack")),
            ..Settings::default()
        })
        .build()
        .with_http(Fake::always(Answer::reply(500, "")) as Arc<dyn Http>)
        .with_random(Arc::new(Chance::cycling()))
}

fn event(happened: Happened, name: &str, reason: Option<&str>) -> Event {
    Event {
        event: happened,
        reference: Some("42".to_owned()),
        name: name.to_owned(),
        reason: reason.map(str::to_owned),
        cleared: false,
    }
}

#[tokio::test]
async fn an_event_is_taken_only_with_the_key_its_service_was_handed_and_owed_to_back_ends() {
    let ctx = machine("notify-events");
    let _ = asked(
        &ctx,
        Asked::Add(Box::new(Adding {
            id: "home".to_owned(),
            kind: BackendKind::Ntfy,
            url: "http://ntfy.home".to_owned(),
            topic: Some("stack".to_owned()),
            token: None,
            reader: None,
            hosted: false,
            attempts: None,
            delays: None,
        })),
    )
    .await;
    let key = minted(&ctx, "sonarr").unwrap_or_default();
    let failed = event(Happened::Failed, "Severance", Some("no release found"));

    let refused = intake(&ctx, "not-the-key", &failed);
    let taken = intake(&ctx, &key, &failed);

    assert_eq!(
        refused.err().map(|problem| problem.code),
        Some(EVENTS_REFUSED)
    );
    assert_eq!(taken.ok().as_deref(), Some("sonarr"));
    let owed = backends::at(&ctx)
        .and_then(|path| backends::kept(&path).ok())
        .and_then(|kept| kept.backends.into_iter().next())
        .map(|one| one.owed)
        .unwrap_or_default();
    assert_eq!(
        owed.iter()
            .map(|one| (one.kind, one.summary.clone()))
            .collect::<Vec<_>>(),
        [(
            Notification::Problem,
            "sonarr could not fetch or import a title".to_owned()
        )]
    );
}

#[test]
fn a_fresh_key_replaces_the_last_one_handed_out() {
    let ctx = machine("notify-events-rekeyed").with_random(Arc::new(Turning(AtomicU8::new(1))));
    let first = minted(&ctx, "jellyfin").unwrap_or_default();
    let second = minted(&ctx, "jellyfin").unwrap_or_default();
    let added = event(Happened::Added, "Dune", None);

    assert_ne!(first, second);
    assert!(intake(&ctx, &first, &added).is_err());
    assert!(intake(&ctx, &second, &added).is_ok());
}

#[test]
fn each_happening_is_told_in_words_at_its_severity() {
    let health = Event {
        cleared: true,
        ..event(Happened::Health, "Indexers are unavailable", None)
    };
    for (happened, expected) in [
        (
            told("sonarr", &event(Happened::Imported, "Severance", None)),
            (
                "import.succeeded",
                Severity::Advisory,
                "sonarr brought a title into the library",
            ),
        ),
        (
            told("jellyfin", &event(Happened::Added, "Dune", None)),
            (
                "import.succeeded",
                Severity::Advisory,
                "jellyfin has a new title in its library",
            ),
        ),
        (
            told("sonarr", &event(Happened::Failed, "Severance", None)),
            (
                "import.failed",
                Severity::Warning,
                "sonarr could not fetch or import a title",
            ),
        ),
        (
            told(
                "sonarr",
                &event(Happened::Health, "Indexers are unavailable", None),
            ),
            (
                "service.health",
                Severity::Warning,
                "sonarr: Indexers are unavailable",
            ),
        ),
        (
            told("sonarr", &health),
            (
                "service.health",
                Severity::Advisory,
                "sonarr cleared: Indexers are unavailable",
            ),
        ),
    ] {
        assert_eq!((happened.0, happened.1, happened.2.as_str()), expected);
    }
    let long = told("sonarr", &event(Happened::Failed, &"x".repeat(500), None));
    assert!(long.2.chars().count() < 220, "{}", long.2);
}
