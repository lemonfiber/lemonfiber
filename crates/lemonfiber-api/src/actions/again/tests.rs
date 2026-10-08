use std::sync::Arc;
use std::time::{Duration, SystemTime};

use axum::body::to_bytes;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use lemonfiber_fixtures::ports::Chance;

use lemonfiber_core::config::Resending;

use super::{Answer, Answered, Asked, Claim, Key, Slot, HEADER, LONGEST};
use crate::actions::Arguments;
use crate::admission::Caller;
use crate::jobs::Job;
use crate::refusal::Refusal;

/// How long an attempt is remembered here: a window of the operator's choosing.
const WITHIN: Duration = Duration::from_secs(5 * 60);

/// How many attempts one caller has remembered here.
const AT_MOST: usize = 3;

/// Both, as the settings would hand them over.
fn bounds() -> Resending {
    Resending {
        within: WITHIN,
        at_most: AT_MOST,
    }
}

/// The moment a test's first send arrives.
fn moment() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000)
}

/// A key a test sends, by the text it carries.
fn key(text: &str) -> Key {
    let Some(key) = Key::read(text.as_bytes()) else {
        unreachable!("a test's key is visible characters");
    };
    key
}

/// What a restart with nothing named asks for.
fn restarting() -> Asked {
    Asked::of("restart", &Arguments::default())
}

/// Headers carrying each of these values under the key's header.
fn carrying(values: &[&[u8]]) -> HeaderMap {
    let mut headers = HeaderMap::new();
    for value in values {
        let Ok(value) = HeaderValue::from_bytes(value) else {
            unreachable!("a test's header value is one a header can carry");
        };
        headers.append(HEADER, value);
    }
    headers
}

/// The slot a claim handed back, or nothing for a key reused.
fn slot(claim: Claim) -> Option<(bool, Slot)> {
    match claim {
        Claim::First(slot) => Some((true, slot)),
        Claim::Again(slot) => Some((false, slot)),
        Claim::Otherwise => None,
    }
}

/// Claim `text` for `who` at `now`, answered at once, so it may be let go.
async fn answered(register: &Answered, who: &Caller, text: &str, now: SystemTime) {
    if let Some((_, slot)) = slot(
        register
            .claim(who, key(text), restarting(), (now, bounds()))
            .await,
    ) {
        let _ = slot.set(Answer::Now(StatusCode::OK, None));
    }
}

/// Whether claiming `text` for `who` at `now` is a first send.
async fn first(register: &Answered, who: &Caller, text: &str, now: SystemTime) -> bool {
    slot(
        register
            .claim(who, key(text), restarting(), (now, bounds()))
            .await,
    )
    .is_some_and(|(first, _)| first)
}

#[test]
fn a_request_without_the_header_carries_no_key() {
    assert_eq!(Key::carried(&HeaderMap::new()), Ok(None));
}

#[test]
fn a_key_given_once_is_read_as_it_was_sent() {
    let headers = carrying(&[b"6f1c-attempt"]);
    assert_eq!(Key::carried(&headers), Ok(Some(key("6f1c-attempt"))));
}

#[test]
fn a_key_given_twice_is_refused_rather_than_read_as_either() {
    let headers = carrying(&[b"one", b"one"]);
    assert_eq!(Key::carried(&headers), Err(Refusal::NotAnIdempotencyKey));
}

#[test]
fn a_key_that_is_not_visible_characters_is_refused() {
    for value in [&b"with space"[..], b"na\xc3\xafve", b""] {
        assert_eq!(
            Key::carried(&carrying(&[value])),
            Err(Refusal::NotAnIdempotencyKey),
            "{value:?}"
        );
    }
}

#[test]
fn a_key_is_read_up_to_its_longest_and_not_a_character_past_it() {
    let longest = "k".repeat(LONGEST);
    assert!(Key::read(longest.as_bytes()).is_some());
    assert!(Key::read(format!("{longest}k").as_bytes()).is_none());
}

#[test]
fn what_an_attempt_asked_for_differs_by_action_and_by_argument() {
    let naming = Arguments {
        forms: vec!["tv".to_owned()],
        ..Arguments::default()
    };
    assert_eq!(restarting(), restarting());
    assert_ne!(restarting(), Asked::of("pull", &Arguments::default()));
    assert_ne!(restarting(), Asked::of("restart", &naming));
}

#[tokio::test]
async fn an_answer_is_replied_as_the_first_send_was() {
    let now = Answer::Now(StatusCode::CONFLICT, Some("{}".to_owned())).reply();
    assert_eq!(now.status(), StatusCode::CONFLICT);
    let body = to_bytes(now.into_body(), usize::MAX)
        .await
        .unwrap_or_default();
    assert_eq!(&body[..], b"{}");
    let Some(job) = Job::mint(&Chance::cycling()) else {
        unreachable!("cycling letters always supply bytes");
    };
    let later = Answer::Later(job.clone(), "restart".to_owned()).reply();
    assert_eq!(later.status(), StatusCode::ACCEPTED);
    let body = to_bytes(later.into_body(), usize::MAX)
        .await
        .unwrap_or_default();
    assert!(String::from_utf8_lossy(&body).contains(job.as_str()));
}

#[tokio::test]
async fn the_same_attempt_sent_again_is_handed_the_first_sends_slot() {
    let register = Answered::default();
    let sent = slot(
        register
            .claim(
                &Caller::Machine,
                key("a"),
                restarting(),
                (moment(), bounds()),
            )
            .await,
    );
    let again = slot(
        register
            .claim(
                &Caller::Machine,
                key("a"),
                restarting(),
                (moment(), bounds()),
            )
            .await,
    );
    let (Some((true, sent)), Some((false, again))) = (sent, again) else {
        unreachable!("one first send and one second");
    };
    assert!(Arc::ptr_eq(&sent, &again));
}

#[tokio::test]
async fn a_key_reused_for_something_else_is_neither_sent_nor_again() {
    let register = Answered::default();
    assert!(first(&register, &Caller::Machine, "a", moment()).await);
    let pulling = Asked::of("pull", &Arguments::default());
    let reused = register
        .claim(&Caller::Machine, key("a"), pulling, (moment(), bounds()))
        .await;
    assert!(matches!(reused, Claim::Otherwise));
}

#[tokio::test]
async fn another_callers_key_is_their_own() {
    let register = Answered::default();
    assert!(first(&register, &Caller::Machine, "a", moment()).await);
    assert!(first(&register, &Caller::Operator, "a", moment()).await);
    assert!(first(&register, &Caller::Member("ana".to_owned()), "a", moment()).await);
}

#[tokio::test]
async fn a_second_send_waits_for_the_answer_the_first_puts_in_its_slot() {
    let register = Answered::default();
    let sent = slot(
        register
            .claim(
                &Caller::Machine,
                key("a"),
                restarting(),
                (moment(), bounds()),
            )
            .await,
    );
    let again = slot(
        register
            .claim(
                &Caller::Machine,
                key("a"),
                restarting(),
                (moment(), bounds()),
            )
            .await,
    );
    let (Some((_, sent)), Some((_, again))) = (sent, again) else {
        unreachable!("one first send and one second");
    };
    let waiting = tokio::spawn(async move { again.wait().await.clone() });
    tokio::task::yield_now().await;
    assert!(!waiting.is_finished());
    let _ = sent.set(Answer::Now(StatusCode::OK, Some("done".to_owned())));
    let heard = waiting.await.ok();
    assert_eq!(
        heard,
        Some(Answer::Now(StatusCode::OK, Some("done".to_owned())))
    );
}

#[tokio::test]
async fn an_answered_attempt_is_let_go_once_it_has_been_remembered_for_as_long_as_one_is() {
    let register = Answered::default();
    answered(&register, &Caller::Machine, "a", moment()).await;
    let nearly = moment() + WITHIN.saturating_sub(Duration::from_secs(1));
    assert!(!first(&register, &Caller::Machine, "a", nearly).await);
    assert!(first(&register, &Caller::Machine, "a", moment() + WITHIN).await);
}

#[tokio::test]
async fn an_attempt_still_in_flight_is_never_let_go_for_its_age() {
    let register = Answered::default();
    assert!(first(&register, &Caller::Machine, "a", moment()).await);
    let long_after = moment() + WITHIN * 3;
    assert!(!first(&register, &Caller::Machine, "a", long_after).await);
}

#[tokio::test]
async fn a_clock_gone_back_lets_nothing_go() {
    let register = Answered::default();
    answered(&register, &Caller::Machine, "a", moment()).await;
    let earlier = SystemTime::UNIX_EPOCH + Duration::from_secs(1_600_000_000);
    assert!(!first(&register, &Caller::Machine, "a", earlier).await);
}

#[tokio::test]
async fn past_the_most_a_caller_has_remembered_their_oldest_answered_attempt_goes() {
    let register = Answered::default();
    answered(&register, &Caller::Operator, "theirs", moment()).await;
    for one in 0..AT_MOST {
        answered(&register, &Caller::Machine, &format!("k{one}"), moment()).await;
    }
    answered(&register, &Caller::Machine, "one-more", moment()).await;
    assert_eq!(register.0.lock().await.len(), AT_MOST + 1);
    assert!(first(&register, &Caller::Machine, "k0", moment()).await);
    assert!(!first(&register, &Caller::Machine, "k2", moment()).await);
    assert!(!first(&register, &Caller::Operator, "theirs", moment()).await);
    assert_eq!(register.0.lock().await.len(), AT_MOST + 1);
}

#[tokio::test]
async fn an_attempt_still_in_flight_is_never_let_go_to_make_room() {
    let register = Answered::default();
    for one in 0..AT_MOST {
        assert!(first(&register, &Caller::Machine, &format!("k{one}"), moment()).await);
    }
    assert!(first(&register, &Caller::Machine, "one-more", moment()).await);
    assert!(!first(&register, &Caller::Machine, "k0", moment()).await);
    assert_eq!(register.0.lock().await.len(), AT_MOST + 1);
}

#[tokio::test]
async fn work_that_finished_is_put_in_the_first_sends_slot_and_kept() {
    let register = Answered::default();
    let sent = slot(
        register
            .claim(
                &Caller::Machine,
                key("a"),
                restarting(),
                (moment(), bounds()),
            )
            .await,
    );
    let Some((true, sent)) = sent else {
        unreachable!("a first send");
    };
    let work = tokio::spawn(async { Answer::Now(StatusCode::OK, Some("done".to_owned())) });
    register.settle(&sent, work).await;
    assert_eq!(
        sent.get(),
        Some(&Answer::Now(StatusCode::OK, Some("done".to_owned())))
    );
    assert!(!first(&register, &Caller::Machine, "a", moment()).await);
}

#[tokio::test]
async fn work_stopped_before_it_answered_is_answered_as_that_and_its_attempt_forgotten() {
    let register = Answered::default();
    answered(&register, &Caller::Machine, "other", moment()).await;
    let sent = slot(
        register
            .claim(
                &Caller::Machine,
                key("a"),
                restarting(),
                (moment(), bounds()),
            )
            .await,
    );
    let again = slot(
        register
            .claim(
                &Caller::Machine,
                key("a"),
                restarting(),
                (moment(), bounds()),
            )
            .await,
    );
    let (Some((true, sent)), Some((false, again))) = (sent, again) else {
        unreachable!("one first send and one second");
    };
    let waiting = tokio::spawn(async move { again.wait().await.clone() });
    let work = tokio::spawn(std::future::pending::<Answer>());
    work.abort();
    register.settle(&sent, work).await;
    let Some(Answer::Now(status, Some(body))) = waiting.await.ok() else {
        unreachable!("the second send is answered with the refusal");
    };
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(body.contains(Refusal::Unanswered.code().as_str()), "{body}");
    assert!(first(&register, &Caller::Machine, "a", moment()).await);
    assert!(!first(&register, &Caller::Machine, "other", moment()).await);
}

#[test]
fn an_attempt_is_remembered_by_default_for_as_long_as_a_jobs_name_is_kept() {
    assert!(Resending::default().within >= crate::jobs::LEASE);
}
