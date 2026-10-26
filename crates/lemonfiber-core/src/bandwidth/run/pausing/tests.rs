use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer as Replies, Fake};

use super::pausing;
use crate::bandwidth::{Declared, Pausing, Pulling};
use crate::config::Settings;
use crate::ports::service::Pulling as Answered;
use crate::test_support::{a_context, env_without_password, json, torrent};

/// A torrent client answering over its contract that it is `pulling` whatever it is told,
/// before and after a stop or a resume.
fn a_client(pulling: Answered) -> Arc<Fake> {
    Fake::by_path(vec![
        (
            torrent::operation("pulling"),
            Replies::reply(200, json(&pulling)),
        ),
        (
            torrent::operation("stop"),
            Replies::reply(200, json(&pulling)),
        ),
        (
            torrent::operation("resume"),
            Replies::reply(200, json(&pulling)),
        ),
    ])
}

/// A stack over `http`, its torrent client moved into its plugin, keeping its files in a
/// directory of this case's own.
fn a_stack(scratch: &str, http: Arc<Fake>) -> crate::app::Ctx {
    let env = env_without_password(&format!("pausing-{scratch}"));
    torrent::reaching(
        a_context()
            .settings(Settings {
                env_file: Some(env),
                ..Settings::default()
            })
            .build()
            .with_http(http),
    )
}

/// The torrent client's line of a report.
fn torrent_line(pauses: &crate::bandwidth::Pauses) -> Option<&crate::bandwidth::Paused> {
    pauses
        .clients
        .iter()
        .find(|client| client.client == torrent::ADAPTER)
}

#[tokio::test]
async fn a_pause_asks_every_client_and_reports_what_each_read_back() {
    let transport = a_client(Answered::Stopped);
    let ctx = a_stack("paused", transport.clone());
    let paused = pausing(&ctx, Pausing::Pause, None).await;
    assert!(transport.asked_for(torrent::operation("stop")));
    assert!(paused.is_ok_and(|paused| torrent_line(&paused)
        .is_some_and(|client| client.now == Some(Pulling::Stopped) && client.unreached.is_none())
        && !paused.rehearsed));
}

#[tokio::test]
async fn a_client_that_reads_back_fetching_is_named_as_fetching_rather_than_counted_paused() {
    // The read-back is the answer, never an echo of the request: a client that took
    // the request and went on starting what it was handed has not been paused.
    let transport = a_client(Answered::Fetching);
    let ctx = a_stack("ignored", transport);
    let paused = pausing(&ctx, Pausing::Pause, None).await;
    assert!(paused.is_ok_and(|paused| torrent_line(&paused)
        .is_some_and(|client| client.now == Some(Pulling::Fetching))
        && !paused.whole()));
}

#[tokio::test]
async fn a_resume_lets_every_client_fetch_again() {
    let transport = a_client(Answered::Fetching);
    let ctx = a_stack("resumed", transport.clone());
    let resumed = pausing(&ctx, Pausing::Resume, None).await;
    assert!(transport.asked_for(torrent::operation("resume")));
    assert!(!transport.asked_for(torrent::operation("stop")));
    assert!(resumed.is_ok_and(|resumed| torrent_line(&resumed)
        .is_some_and(|client| client.now == Some(Pulling::Fetching))
        && resumed.caution.is_none()));
}

#[tokio::test]
async fn a_rehearsal_asks_each_client_what_it_is_doing_and_tells_it_nothing() {
    let transport = a_client(Answered::Fetching);
    let ctx = a_stack("rehearsed", transport.clone()).rehearsing();
    let rehearsed = pausing(&ctx, Pausing::Pause, None).await;
    assert!(!transport.asked_for(torrent::operation("stop")));
    assert!(rehearsed.is_ok_and(|rehearsed| torrent_line(&rehearsed)
        .is_some_and(|client| client.was == Some(Pulling::Fetching) && client.now.is_none())));
}

/// A Usenet client moved in whose key this machine never wrote down is named as not
/// reached: an answer that left it out would read as every client paused.
#[tokio::test]
async fn a_client_nothing_here_can_open_is_named_as_not_reached() {
    let ctx = a_stack("unopened", a_client(Answered::Stopped));
    crate::test_support::usenet::unkeyed(&ctx);
    let paused = pausing(&ctx, Pausing::Pause, None).await;
    let clients = paused.map(|paused| paused.clients).unwrap_or_default();
    assert!(
        clients.iter().any(
            |client| client.client == crate::test_support::usenet::ADAPTER
                && client.unreached.is_some()
                && client.now.is_none()
        ),
        "{clients:?}"
    );
}

#[tokio::test]
async fn a_client_that_will_not_answer_is_named_in_its_own_words() {
    let ctx = a_stack("silent", Fake::silent());
    let paused = pausing(&ctx, Pausing::Pause, None).await;
    assert!(paused.is_ok_and(|paused| torrent_line(&paused)
        .is_some_and(|client| client.unreached.is_some() && client.now.is_none())
        && !paused.whole()));
}

#[tokio::test]
async fn a_pause_is_the_operators_so_a_month_turning_over_starts_nothing() {
    // The cap had stopped the clients. Pausing takes that stop over, so the run that
    // starts again what lemonfiber stopped finds nothing of its own to start.
    let transport = a_client(Answered::Stopped);
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
    let ctx = a_stack("taken-over-rehearsed", a_client(Answered::Stopped)).rehearsing();
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
    let ctx = a_stack("resumed-at-the-cap", a_client(Answered::Fetching));
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
    let transport = a_client(Answered::Stopped);
    let ctx = a_context()
        .over(nowhere)
        .build()
        .with_http(transport.clone());
    let refused = pausing(&ctx, Pausing::Resume, None).await;
    assert!(
        refused.is_err_and(|problem| problem.code == crate::error::codes::stack::STACK_UNREADABLE)
    );
    assert!(!transport.asked_for(torrent::operation("resume")));
}

#[tokio::test]
async fn a_rehearsal_names_a_client_that_will_not_say_what_it_is_doing() {
    let ctx = a_stack("silent-rehearsed", Fake::silent()).rehearsing();
    let rehearsed = pausing(&ctx, Pausing::Pause, None).await;
    assert!(rehearsed.is_ok_and(|rehearsed| torrent_line(&rehearsed)
        .is_some_and(|client| client.unreached.is_some() && client.was.is_none())));
}

/// The offer a rehearsal answered, carried back unchanged, is acted on.
#[tokio::test]
async fn a_pause_answering_the_offer_it_was_rehearsed_with_is_carried_out() {
    let transport = a_client(Answered::Stopped);
    let ctx = a_stack("answered", transport.clone());
    let rehearsed = pausing(&ctx, Pausing::Pause, None)
        .await
        .map(|paused| paused.offer);
    let offer = rehearsed.unwrap_or_default();
    let paused = pausing(&ctx, Pausing::Pause, Some(&offer)).await;
    assert!(paused.is_ok_and(|paused| paused.offer == offer));
    assert!(transport.asked_for(torrent::operation("stop")));
}

/// An offer for clients as they no longer are tells no client anything.
#[tokio::test]
async fn a_pause_answering_an_offer_that_moved_tells_no_client_anything() {
    let transport = a_client(Answered::Stopped);
    let ctx = a_stack("moved", transport.clone());
    let refused = pausing(&ctx, Pausing::Pause, Some("ffffffff")).await.err();
    assert_eq!(
        refused.map(|problem| problem.code),
        Some(crate::error::codes::rate::PAUSING_MOVED)
    );
    assert!(!transport.asked_for(torrent::operation("stop")));
}
