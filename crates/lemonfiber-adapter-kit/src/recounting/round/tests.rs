use std::time::Duration;

use axum::http::StatusCode;
use lemonfiber_fixtures::scratch::Scratch;
use lemonfiber_ports::service::Hearing;

use super::super::kept::Kept;
use std::sync::Arc;

use lemonfiber_fixtures::heard::Heard;

use super::super::tests::{kept_in, names, recounter, recounter_heard, Script};
use super::super::told::tests::{address, core, KEY};
use super::super::told::Told;
use super::super::Entry;
use super::{moved, round, sent, Waiting};

fn kept(address: String, pin: String, cursor: &str) -> Kept {
    Kept {
        address,
        pin,
        every: 15,
        hearing: Hearing::Poll,
        cursor: cursor.to_owned(),
        secret: None,
    }
}

#[test]
fn a_poll_waits_its_interval_and_a_push_waits_for_a_push() {
    let mut polled = kept(String::new(), String::new(), "0");
    assert_eq!(
        Waiting::of(&polled),
        Waiting::Every(Duration::from_secs(15))
    );
    polled.hearing = Hearing::Push;
    assert_eq!(Waiting::of(&polled), Waiting::Pushed);
}

#[tokio::test]
async fn a_cursor_the_upstream_no_longer_holds_moves_to_its_newest_and_sends_nothing() {
    let dir = Scratch::new("round-gone");
    let history = Script::holding(&[Some("Dune"), Some("Arrival"), Some("Heat")]);
    history.forgets("1");
    let Some((served, posts)) = core(StatusCode::ACCEPTED) else {
        return;
    };
    let heard = Arc::new(Heard::default());
    let recounter = recounter_heard(dir.path(), &history, &heard);
    let told = Told::new(&address(&served), &served.pin, KEY);
    recounter.shared.running.lock().await.kept =
        Some(kept(address(&served), served.pin.clone(), "1"));
    let finished = match &told {
        Some(told) => round(&recounter.shared, told).await,
        None => false,
    };
    assert!(finished);
    assert_eq!(
        kept_in(dir.path()).map(|kept| kept.cursor).as_deref(),
        Some("3")
    );
    assert!(names(&posts).is_empty());
    assert!(heard
        .said()
        .iter()
        .any(|said| said.contains("no longer holds 1") && said.contains("its newest, 3")));
}

#[tokio::test]
async fn a_history_that_cannot_be_read_ends_the_round_unfinished() {
    let dir = Scratch::new("round-unread");
    let history = Script::holding(&[Some("Dune")]);
    history.fails();
    let Some((served, posts)) = core(StatusCode::ACCEPTED) else {
        return;
    };
    let recounter = recounter(dir.path(), &history);
    let told = Told::new(&address(&served), &served.pin, KEY);
    recounter.shared.running.lock().await.kept =
        Some(kept(address(&served), served.pin.clone(), "0"));
    let finished = match &told {
        Some(told) => round(&recounter.shared, told).await,
        None => true,
    };
    assert!(!finished);
    history.forgets("0");
    let finished = match &told {
        Some(told) => round(&recounter.shared, told).await,
        None => true,
    };
    assert!(!finished);
    assert!(names(&posts).is_empty());
}

#[tokio::test]
async fn a_round_with_nothing_kept_sends_nothing() {
    let dir = Scratch::new("round-unkept");
    let history = Script::holding(&[Some("Dune")]);
    let Some((served, posts)) = core(StatusCode::ACCEPTED) else {
        return;
    };
    let recounter = recounter(dir.path(), &history);
    let told = Told::new(&address(&served), &served.pin, KEY);
    let finished = match &told {
        Some(told) => round(&recounter.shared, told).await,
        None => true,
    };
    assert!(!finished);
    assert!(names(&posts).is_empty());
}

#[tokio::test]
async fn a_cursor_that_cannot_be_kept_ends_the_round_after_the_send() {
    let dir = Scratch::new("round-unkeepable");
    let history = Script::holding(&[Some("Dune"), Some("Arrival")]);
    let Some((served, posts)) = core(StatusCode::ACCEPTED) else {
        return;
    };
    let missing = dir.path().join("not-there");
    let recounter = recounter(&missing, &history);
    let told = Told::new(&address(&served), &served.pin, KEY);
    recounter.shared.running.lock().await.kept =
        Some(kept(address(&served), served.pin.clone(), "0"));
    let finished = match &told {
        Some(told) => round(&recounter.shared, told).await,
        None => true,
    };
    assert!(!finished);
    assert_eq!(names(&posts), ["Dune"]);
}

#[tokio::test]
async fn a_send_the_core_refuses_or_after_a_stop_moves_no_cursor() {
    let dir = Scratch::new("round-unsent");
    let history = Script::holding(&[Some("Dune")]);
    let Some((refusing, refused)) = core(StatusCode::INTERNAL_SERVER_ERROR) else {
        return;
    };
    let recounter = recounter(dir.path(), &history);
    let entry = Entry {
        position: "1".to_owned(),
        event: Some(super::super::told::tests::added("Dune")),
    };
    let Some(told) = Told::new(&address(&refusing), &refusing.pin, KEY) else {
        unreachable!("a fake core's address and pin will do");
    };
    assert!(!sent(&recounter.shared, &told, entry.clone()).await);
    assert!(names(&refused).is_empty());

    recounter.shared.running.lock().await.kept =
        Some(kept(address(&refusing), refusing.pin.clone(), "0"));
    assert!(!sent(&recounter.shared, &told, entry).await);
    assert_eq!(names(&refused), ["Dune"]);
    assert_eq!(
        recounter
            .shared
            .running
            .lock()
            .await
            .kept
            .as_ref()
            .map(|kept| kept.cursor.clone())
            .as_deref(),
        Some("0")
    );
    assert!(kept_in(dir.path()).is_none());
}

#[tokio::test]
async fn a_gone_cursor_is_left_where_the_history_cannot_be_read_or_it_moved_meanwhile() {
    let dir = Scratch::new("round-gone-unmoved");
    let history = Script::holding(&[Some("Dune"), Some("Arrival")]);
    let heard = Arc::new(Heard::default());
    let recounter = recounter_heard(dir.path(), &history, &heard);

    assert!(!moved(&recounter.shared, "1").await);

    recounter.shared.running.lock().await.kept = Some(kept(String::new(), String::new(), "2"));
    assert!(!moved(&recounter.shared, "1").await);
    assert_eq!(
        recounter
            .shared
            .running
            .lock()
            .await
            .kept
            .as_ref()
            .map(|kept| kept.cursor.clone())
            .as_deref(),
        Some("2")
    );

    history.fails();
    assert!(!moved(&recounter.shared, "2").await);
    assert!(kept_in(dir.path()).is_none());
    assert!(heard
        .said()
        .iter()
        .any(|said| said.contains("its history could not be read")));
    assert!(!heard
        .said()
        .iter()
        .any(|said| said.contains("no longer holds")));
}
