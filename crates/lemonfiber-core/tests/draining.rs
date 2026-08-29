//! A teardown that lets the downloads finish, and a start of named services.
//!
//! Both are dispatched here rather than from a `#[cfg(test)]` module because both
//! are `async` and reach the download clients, and an async path exercised only
//! in-crate has its coverage counted from the copy that never ran.
//!
//! The wait is elapsed in virtual time. It looks again every ten seconds, and a
//! test that really waited would be one nobody runs — what matters is that it looks
//! again at all, and that it stops when there is nothing left to wait for.

mod common;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use common::stack::project;
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome, Waiting};
use lemonfiber_core::config::{store, Protocols, Settings, QBITTORRENT_PASSWORD_KEY};
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::docker::{Health, Lifecycle};
use lemonfiber_core::ports::http::Http;
use lemonfiber_core::ports::Narrator;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::downloads::{QBIT_FINISHED, QBIT_TORRENTS, SAB_EMPTY};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::{a_password, spoke, Reporting, Scripted};
use tokio::sync::Mutex;

/// The services the `dl` form declares — the two download clients and the tunnel
/// one of them runs behind.
const DOWNLOADING: [&str; 3] = ["sabnzbd", "gluetun", "qbittorrent"];

/// Everything a wait said, in the order it said it.
#[derive(Default)]
struct Heard(Mutex<Vec<String>>);

#[async_trait]
impl Narrator for Heard {
    async fn say(&self, said: &str) {
        self.0.lock().await.push(said.to_owned());
    }
}

impl Heard {
    /// What it has heard so far.
    async fn said(&self) -> Vec<String> {
        self.0.lock().await.clone()
    }
}

/// A private environment file recording qBittorrent's password, at a scratch path
/// unique to this case so concurrent tests do not share one.
fn env_at(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lemonfiber-drain-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join(".env");
    assert!(
        store::set(&path, QBITTORRENT_PASSWORD_KEY, &a_password()).is_ok(),
        "the scratch environment file is written"
    );
    path
}

/// A context reaching the download clients over `http`, against the stack this
/// repository carries, saying what it waits for through `heard`.
fn ctx(http: Arc<dyn Http>, name: &str, heard: &Arc<Heard>) -> Ctx {
    Ctx::new(
        Arc::new(Scripted(Ok(spoke("")))),
        Arc::new(Reporting::holding(
            &DOWNLOADING,
            Lifecycle::Running,
            Health::Healthy,
        )),
        lemonfiber_fixtures::ports::Stopped::today(),
        lemonfiber_fixtures::files::Files::empty(),
        Source::External(project()),
        Settings {
            protocols: Protocols::both(),
            env_file: Some(env_at(name)),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_http(http)
    .narrating(Arc::clone(heard) as Arc<dyn Narrator>)
    .waiting(Duration::ZERO)
}

/// The forms an operator names.
fn named(forms: &[&str]) -> Vec<String> {
    forms.iter().map(|form| (*form).to_owned()).collect()
}

/// The envelope a dispatched command renders, or nothing where it refused.
fn rendered(outcome: Result<Outcome, Box<lemonfiber_core::error::Problem>>) -> Option<String> {
    outcome
        .ok()
        .and_then(|outcome| outcome.envelope().to_json())
}

#[tokio::test(start_paused = true)]
async fn a_teardown_asked_to_wait_holds_on_until_nothing_is_coming_down() {
    // qBittorrent answers with the same torrent still coming down twice, and then
    // with one that has finished: something to wait for, a look that has no news,
    // and an end. `SABnzbd` has no key on this filesystem and contributes nothing.
    let fake = Fake::by_path_in_turn(vec![
        ("/auth/login", vec![Answer::reply(200, "Ok.")]),
        (
            "/torrents/info",
            vec![
                Answer::reply(200, QBIT_TORRENTS),
                Answer::reply(200, QBIT_TORRENTS),
                Answer::reply(200, QBIT_FINISHED),
            ],
        ),
        ("", vec![Answer::reply(200, SAB_EMPTY)]),
    ]);
    let heard = Arc::new(Heard::default());
    let context = ctx(Arc::clone(&fake) as Arc<dyn Http>, "waiting", &heard);

    let envelope = rendered(
        dispatch(
            Command::Down {
                forms: named(&["dl"]),
                wait: Waiting::ForTheDownloads,
            },
            &context,
        )
        .await,
    );

    let said = heard.said().await;
    // Once, not once per look: a line repeated every ten seconds is one whoever is
    // reading scrolls past, and the moment it has news is the moment a count changes.
    assert_eq!(
        said.iter()
            .filter(|line| line.contains("waiting for 1 download to finish"))
            .count(),
        1,
        "the wait says what it is waiting for, and says it once: {said:?}"
    );
    assert_eq!(
        said.last().map(String::as_str),
        Some("downloads finished — stopping now"),
        "and says so when there is nothing left: {said:?}"
    );
    assert!(
        envelope.is_some_and(|rendered| rendered.contains("\"action\":\"down\"")),
        "and the teardown it was in front of still happened"
    );
}

#[tokio::test]
async fn a_teardown_that_was_not_asked_to_wait_asks_the_clients_nothing() {
    // The ordinary teardown. Going to the network to discover there is nothing to
    // wait for would put a request on the common path for the sake of a question
    // nobody asked.
    let fake = Fake::silent();
    let heard = Arc::new(Heard::default());
    let context = ctx(Arc::clone(&fake) as Arc<dyn Http>, "not-waiting", &heard);

    let envelope = rendered(
        dispatch(
            Command::Down {
                forms: named(&["dl"]),
                wait: Waiting::Never,
            },
            &context,
        )
        .await,
    );

    assert!(envelope.is_some(), "the teardown happened");
    assert!(
        fake.requests().is_empty(),
        "and no download client was asked anything"
    );
    assert!(heard.said().await.is_empty(), "and nothing was narrated");
}

#[tokio::test]
async fn starting_named_services_runs_the_start_compose_spells_that_way() {
    let heard = Arc::new(Heard::default());
    let context = ctx(Fake::silent(), "start", &heard);

    let envelope = rendered(
        dispatch(
            Command::Start {
                forms: named(&["dl"]),
                services: vec!["qbittorrent".to_owned()],
            },
            &context,
        )
        .await,
    );

    let envelope = envelope.unwrap_or_default();
    assert!(
        envelope.contains("\"action\":\"up\""),
        "it is a start, and reports as one: {envelope}"
    );
    assert!(
        envelope.contains("start") && envelope.contains("qbittorrent"),
        "and the command it ran names the services rather than the form: {envelope}"
    );
}
