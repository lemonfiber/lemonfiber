use std::sync::Arc;

use super::{Http, Method, Request, Retrying};
use lemonfiber_error::retry::ATTEMPTS;
use lemonfiber_fixtures::http::{Answer, Fake};

/// What a service that is up answers.
fn answered() -> Answer {
    Answer::reply(200, "answered")
}

/// A transport that fails the first `failures` times and answers after that, wrapped, and
/// the fake the test counts its asks through — shared rather than borrowed, since the
/// wrapper takes ownership.
fn failing(failures: usize) -> (Retrying<Arc<dyn Http>>, Arc<Fake>) {
    let mut answers = vec![Answer::Silent; failures];
    answers.push(answered());
    wrapped(Fake::in_turn(answers))
}

/// A transport that fails every time it is asked, wrapped.
fn down() -> (Retrying<Arc<dyn Http>>, Arc<Fake>) {
    wrapped(Fake::silent())
}

/// `fake`, wrapped, and `fake` again to count its asks through.
fn wrapped(fake: Arc<Fake>) -> (Retrying<Arc<dyn Http>>, Arc<Fake>) {
    (Retrying::around(Arc::clone(&fake) as Arc<dyn Http>), fake)
}

/// How many times `fake` was asked.
fn asked(fake: &Fake) -> u32 {
    u32::try_from(fake.requests().len()).unwrap_or(u32::MAX)
}

/// A request by method, to a service that does not matter here.
fn request(method: Method) -> Request {
    Request {
        method,
        url: "http://sonarr:8989/api/v3/system/status".to_owned(),
        headers: Vec::new(),
        body: None,
        pinned: None,
    }
}

#[tokio::test]
async fn a_service_that_was_only_starting_is_never_reported_at_all() {
    // The case the whole thing exists for: it failed once, it works now, and an
    // operator who was told about it would have learnt that lemonfiber cries wolf.
    let (retrying, fake) = failing(1);
    assert!(retrying.send(&request(Method::Get)).await.is_ok());
    assert_eq!(asked(&fake), 2);
}

#[tokio::test]
async fn a_service_that_is_down_is_reported_with_how_hard_it_was_tried() {
    let (retrying, fake) = down();
    let reported = retrying.send(&request(Method::Get)).await;
    assert_eq!(
        reported
            .err()
            .map(|failure| (failure.attempts, failure.reason)),
        Some((ATTEMPTS, "connection refused".to_owned())),
        "the transport's own words, and our count beside them"
    );
    assert_eq!(
        asked(&fake),
        ATTEMPTS,
        "tried to exhaustion, and no further"
    );
}

#[tokio::test]
async fn a_create_that_went_unanswered_is_never_sent_twice() {
    // The response is what went missing, not necessarily the request — so a
    // retry risks a second root folder, a second download client.
    let (retrying, fake) = failing(1);
    assert!(retrying.send(&request(Method::Post)).await.is_err());
    assert_eq!(asked(&fake), 1, "asked once and only once");
}

#[tokio::test]
async fn a_replace_is_safe_to_repeat_and_so_is_retried() {
    // It lands on the same state however many times it arrives, which is the
    // property that makes trying again safe.
    let (retrying, fake) = failing(1);
    assert!(retrying.send(&request(Method::Put)).await.is_ok());
    assert_eq!(asked(&fake), 2);
}

#[tokio::test]
async fn a_failure_nobody_retried_says_it_was_tried_once() {
    // Not zero, and not the full count: claiming persistence for something
    // nobody retried is the same overstatement in the other direction.
    let (retrying, _) = down();
    let reported = retrying.send(&request(Method::Post)).await;
    assert_eq!(reported.err().map(|failure| failure.attempts), Some(1));
}

#[tokio::test]
async fn a_service_that_answers_first_time_is_asked_once() {
    // No wait, no second call: the ordinary path pays nothing for this.
    let (retrying, fake) = failing(0);
    assert!(retrying.send(&request(Method::Get)).await.is_ok());
    assert_eq!(asked(&fake), 1);
}

#[tokio::test]
async fn a_refusal_is_an_answer_and_is_never_retried() {
    // Asking a service that said `401` again three times is neither more polite
    // nor more informative.
    let (retrying, fake) = wrapped(Fake::always(Answer::reply(401, "no")));
    let answered = retrying.send(&request(Method::Get)).await;
    assert!(answered.is_ok_and(|response| response.status == 401));
    assert_eq!(asked(&fake), 1);
}

/// Asking again would spend the whole wait twice more to hear the same silence.
#[tokio::test]
async fn a_service_that_took_the_connection_and_said_nothing_is_asked_once() {
    let (retrying, fake) = wrapped(Fake::always(Answer::went_quiet("operation timed out")));
    let failure = retrying.send(&request(Method::Get)).await.err();
    assert_eq!(asked(&fake), 1);
    assert_eq!(failure.map(|failure| failure.attempts), Some(1));
}

#[tokio::test]
async fn a_fetch_is_handed_to_what_it_wraps() {
    let (transport, fake) = failing(0);
    let fetched = transport.fetch(&request(Method::Get), 64).await;
    assert_eq!(
        fetched.map(|fetched| fetched.bytes),
        Ok(Some(b"answered".to_vec()))
    );
    assert_eq!(asked(&fake), 1);
}

/// Octets are handed to what is wrapped once, as a create is: a message sent twice would
/// wake a phone twice.
#[tokio::test]
async fn octets_are_handed_on_once() {
    let (transport, fake) = failing(0);
    let sent = transport
        .send_octets(&request(Method::Post), b"sealed")
        .await;
    assert_eq!(sent.map(|answered| answered.status), Ok(200));
    assert_eq!(asked(&fake), 1);
}
