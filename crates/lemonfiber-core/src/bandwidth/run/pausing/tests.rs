use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer as Replies, Fake};

use super::pausing;
use crate::bandwidth::{Declared, Pausing, Pulling};
use crate::config::Settings;
use crate::test_support::{a_context, a_password, env_at};

/// What qBittorrent says about whether it would start the next thing it is handed.
const ADDS_STOPPED: &str = r#"{"add_stopped_enabled":true}"#;

/// The same, where it would start it.
const ADDS_RUNNING: &str = r#"{"add_stopped_enabled":false}"#;

/// A torrent client that answers every question this asks, reading back `preferences`
/// after being told, with the torrents named in `running` still going.
fn a_client(preferences: &'static str, running: &'static str) -> Arc<Fake> {
    Fake::by_path(vec![
        ("/api/v2/auth/login", Replies::reply(200, "Ok.")),
        (
            "/api/v2/app/setPreferences",
            Replies::reply(200, String::new()),
        ),
        ("/api/v2/app/preferences", Replies::reply(200, preferences)),
        ("/api/v2/torrents/info", Replies::reply(200, running)),
        ("/api/v2/torrents/stop", Replies::reply(200, String::new())),
        ("/api/v2/torrents/start", Replies::reply(200, String::new())),
    ])
}

/// A stack over `http`, keeping its files in a directory of this case's own.
fn a_stack(scratch: &str, http: Arc<Fake>) -> crate::app::Ctx {
    let env = env_at(&format!("pausing-{scratch}"), &a_password());
    a_context()
        .settings(Settings {
            env_file: Some(env),
            ..Settings::default()
        })
        .build()
        .with_http(http)
}

/// The torrent client's line of a report.
fn torrent(pauses: &crate::bandwidth::Pauses) -> Option<&crate::bandwidth::Paused> {
    pauses
        .clients
        .iter()
        .find(|client| client.client == crate::qbittorrent::SERVICE)
}

#[tokio::test]
async fn a_pause_asks_every_client_and_reports_what_each_read_back() {
    let transport = a_client(ADDS_STOPPED, "[]");
    let ctx = a_stack("paused", transport.clone());
    let paused = pausing(&ctx, Pausing::Pause, None).await;
    assert!(transport.asked_for("/torrents/stop"));
    assert!(paused.is_ok_and(|paused| torrent(&paused)
        .is_some_and(|client| client.now == Some(Pulling::Stopped) && client.unreached.is_none())
        && !paused.rehearsed));
}

#[tokio::test]
async fn a_client_that_reads_back_fetching_is_named_as_fetching_rather_than_counted_paused() {
    // The read-back is the answer, never an echo of the request: a client that took
    // the request and went on starting what it was handed has not been paused.
    let transport = a_client(ADDS_RUNNING, "[]");
    let ctx = a_stack("ignored", transport);
    let paused = pausing(&ctx, Pausing::Pause, None).await;
    assert!(paused.is_ok_and(|paused| torrent(&paused)
        .is_some_and(|client| client.now == Some(Pulling::Fetching))
        && !paused.whole()));
}

#[tokio::test]
async fn a_resume_lets_every_client_fetch_again() {
    let transport = a_client(ADDS_RUNNING, "[]");
    let ctx = a_stack("resumed", transport.clone());
    let resumed = pausing(&ctx, Pausing::Resume, None).await;
    assert!(transport.asked_for("/torrents/start"));
    assert!(!transport.asked_for("/torrents/stop"));
    assert!(resumed.is_ok_and(|resumed| torrent(&resumed)
        .is_some_and(|client| client.now == Some(Pulling::Fetching))
        && resumed.caution.is_none()));
}

#[tokio::test]
async fn a_rehearsal_asks_each_client_what_it_is_doing_and_tells_it_nothing() {
    let transport = a_client(ADDS_RUNNING, "[]");
    let ctx = a_stack("rehearsed", transport.clone()).rehearsing();
    let rehearsed = pausing(&ctx, Pausing::Pause, None).await;
    assert!(!transport.asked_for("/torrents/stop"));
    assert!(!transport.asked_for("setPreferences"));
    assert!(rehearsed.is_ok_and(|rehearsed| torrent(&rehearsed)
        .is_some_and(|client| client.was == Some(Pulling::Fetching) && client.now.is_none())));
}

#[tokio::test]
async fn a_client_nothing_here_can_open_is_named_as_not_reached() {
    // The fixture stack declares a Usenet client whose key this machine never wrote
    // down. An answer that left it out would read as every client paused.
    let ctx = a_stack("unopened", a_client(ADDS_STOPPED, "[]"));
    let paused = pausing(&ctx, Pausing::Pause, None).await;
    assert!(paused.is_ok_and(|paused| paused
        .clients
        .iter()
        .any(|client| client.client == crate::sabnzbd::SERVICE
            && client.unreached.is_some()
            && client.now.is_none())));
}

#[tokio::test]
async fn a_client_that_will_not_answer_is_named_in_its_own_words() {
    let ctx = a_stack("silent", Fake::silent());
    let paused = pausing(&ctx, Pausing::Pause, None).await;
    assert!(paused.is_ok_and(|paused| torrent(&paused)
        .is_some_and(|client| client.unreached.is_some() && client.now.is_none())
        && !paused.whole()));
}

#[tokio::test]
async fn a_pause_is_the_operators_so_a_month_turning_over_starts_nothing() {
    // The cap had stopped the clients. Pausing takes that stop over, so the run that
    // starts again what lemonfiber stopped finds nothing of its own to start.
    let transport = a_client(ADDS_STOPPED, "[]");
    let ctx = a_stack("taken-over", transport);
    crate::app::record::keep_beside(
        &ctx,
        super::super::RECORD,
        &Declared {
            stopped: true,
            ..Declared::default()
        },
    );
    assert!(pausing(&ctx, Pausing::Pause, None).await.is_ok());
    assert!(!super::super::recorded(&ctx).stopped);
}

#[tokio::test]
async fn a_rehearsed_pause_leaves_the_caps_stop_where_it_was() {
    let ctx = a_stack("taken-over-rehearsed", a_client(ADDS_STOPPED, "[]")).rehearsing();
    crate::app::record::keep_beside(
        &ctx,
        super::super::RECORD,
        &Declared {
            stopped: true,
            ..Declared::default()
        },
    );
    assert!(pausing(&ctx, Pausing::Pause, None).await.is_ok());
    assert!(super::super::recorded(&ctx).stopped);
}

#[tokio::test]
async fn a_resume_says_where_a_spent_cap_will_stop_the_clients_again() {
    let ctx = a_stack("resumed-at-the-cap", a_client(ADDS_RUNNING, "[]"));
    crate::app::record::keep_beside(
        &ctx,
        super::super::RECORD,
        &Declared {
            stopped: true,
            ..Declared::default()
        },
    );
    let resumed = pausing(&ctx, Pausing::Resume, None).await;
    assert!(resumed.is_ok_and(|resumed| resumed
        .caution
        .is_some_and(|said| said.contains("cap is spent"))));
}

#[test]
fn a_stack_with_no_download_client_is_refused_by_the_request() {
    for asked in [Pausing::Pause, Pausing::Resume] {
        let problem = super::nothing_to_pause(asked);
        assert_eq!(problem.code, crate::error::codes::rate::NOTHING_TO_PAUSE);
        assert!(problem.summary.ends_with(asked.word()));
    }
}

#[test]
fn a_stack_declaring_no_download_client_asks_nobody() {
    for asked in [Pausing::Pause, Pausing::Resume] {
        let refused = super::declaring(&[], asked);
        assert!(refused
            .is_err_and(|problem| problem.code == crate::error::codes::rate::NOTHING_TO_PAUSE));
    }
}

#[tokio::test]
async fn a_stack_that_cannot_be_read_asks_nobody() {
    let nowhere = crate::stack::Source::External(std::path::Path::new("/lemonfiber/no/such/stack"));
    let transport = a_client(ADDS_STOPPED, "[]");
    let ctx = a_context()
        .over(nowhere)
        .build()
        .with_http(transport.clone());
    let refused = pausing(&ctx, Pausing::Resume, None).await;
    assert!(
        refused.is_err_and(|problem| problem.code == crate::error::codes::stack::STACK_UNREADABLE)
    );
    assert!(!transport.asked_for("/torrents/start"));
}

#[tokio::test]
async fn a_rehearsal_names_a_client_that_will_not_say_what_it_is_doing() {
    let ctx = a_stack("silent-rehearsed", Fake::silent()).rehearsing();
    let rehearsed = pausing(&ctx, Pausing::Pause, None).await;
    assert!(rehearsed.is_ok_and(|rehearsed| torrent(&rehearsed)
        .is_some_and(|client| client.unreached.is_some() && client.was.is_none())));
}

/// The offer a rehearsal answered, carried back unchanged, is acted on.
#[tokio::test]
async fn a_pause_answering_the_offer_it_was_rehearsed_with_is_carried_out() {
    let transport = a_client(ADDS_STOPPED, "[]");
    let ctx = a_stack("answered", transport.clone());
    let rehearsed = pausing(&ctx, Pausing::Pause, None)
        .await
        .map(|paused| paused.offer);
    let offer = rehearsed.unwrap_or_default();
    let paused = pausing(&ctx, Pausing::Pause, Some(&offer)).await;
    assert!(paused.is_ok_and(|paused| paused.offer == offer));
    assert!(transport.asked_for("/torrents/stop"));
}

/// An offer for clients as they no longer are tells no client anything.
#[tokio::test]
async fn a_pause_answering_an_offer_that_moved_tells_no_client_anything() {
    let transport = a_client(ADDS_STOPPED, "[]");
    let ctx = a_stack("moved", transport.clone());
    let refused = pausing(&ctx, Pausing::Pause, Some("ffffffff")).await.err();
    assert_eq!(
        refused.map(|problem| problem.code),
        Some(crate::error::codes::rate::PAUSING_MOVED)
    );
    assert!(!transport.asked_for("/torrents/stop"));
}
