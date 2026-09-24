use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use super::{Http, Method, Request, Response, Retrying, Unreachable};
use async_trait::async_trait;
use lemonfiber_error::retry::ATTEMPTS;

/// A transport that fails the first `failures` times and answers after that,
/// counting how many times it was asked.
struct Flaky {
    failures: u32,
    asked: Arc<AtomicU32>,
}

/// A flaky transport, wrapped, and the counter the test reads it through —
/// shared rather than borrowed, since the wrapper takes ownership.
fn failing(failures: u32) -> (Retrying<Flaky>, Arc<AtomicU32>) {
    let asked = Arc::new(AtomicU32::new(0));
    let flaky = Flaky {
        failures,
        asked: Arc::clone(&asked),
    };
    (Retrying::around(flaky), asked)
}

#[async_trait]
impl Http for Flaky {
    async fn send(&self, request: &Request) -> Result<Response, Unreachable> {
        let asked = self.asked.fetch_add(1, Ordering::SeqCst).saturating_add(1);
        if asked <= self.failures {
            return Err(Unreachable::once(&request.url, "connection refused"));
        }
        Ok(Response {
            status: 200,
            headers: Vec::new(),
            body: "answered".to_owned(),
        })
    }
}

/// A request by method, to a service that does not matter here.
fn request(method: Method) -> Request {
    Request {
        method,
        url: "http://sonarr:8989/api/v3/system/status".to_owned(),
        headers: Vec::new(),
        body: None,
    }
}

#[tokio::test]
async fn a_service_that_was_only_starting_is_never_reported_at_all() {
    // The case the whole thing exists for: it failed once, it works now, and an
    // operator who was told about it would have learnt that lemonfiber cries wolf.
    let (retrying, asked) = failing(1);
    assert!(retrying.send(&request(Method::Get)).await.is_ok());
    assert_eq!(asked.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn a_service_that_is_down_is_reported_with_how_hard_it_was_tried() {
    let (retrying, asked) = failing(u32::MAX);
    let reported = retrying.send(&request(Method::Get)).await;
    assert_eq!(
        reported
            .err()
            .map(|failure| (failure.attempts, failure.reason)),
        Some((ATTEMPTS, "connection refused".to_owned())),
        "the transport's own words, and our count beside them"
    );
    assert_eq!(
        asked.load(Ordering::SeqCst),
        ATTEMPTS,
        "tried to exhaustion, and no further"
    );
}

#[tokio::test]
async fn a_create_that_went_unanswered_is_never_sent_twice() {
    // The response is what went missing, not necessarily the request — so a
    // retry risks a second root folder, a second download client.
    let (retrying, asked) = failing(1);
    assert!(retrying.send(&request(Method::Post)).await.is_err());
    assert_eq!(asked.load(Ordering::SeqCst), 1, "asked once and only once");
}

#[tokio::test]
async fn a_replace_is_safe_to_repeat_and_so_is_retried() {
    // It lands on the same state however many times it arrives, which is the
    // property that makes trying again safe.
    let (retrying, asked) = failing(1);
    assert!(retrying.send(&request(Method::Put)).await.is_ok());
    assert_eq!(asked.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn a_failure_nobody_retried_says_it_was_tried_once() {
    // Not zero, and not the full count: claiming persistence for something
    // nobody retried is the same overstatement in the other direction.
    let (retrying, _) = failing(u32::MAX);
    let reported = retrying.send(&request(Method::Post)).await;
    assert_eq!(reported.err().map(|failure| failure.attempts), Some(1));
}

#[tokio::test]
async fn a_service_that_answers_first_time_is_asked_once() {
    // No wait, no second call: the ordinary path pays nothing for this.
    let (retrying, asked) = failing(0);
    assert!(retrying.send(&request(Method::Get)).await.is_ok());
    assert_eq!(asked.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_refusal_is_an_answer_and_is_never_retried() {
    // Asking a service that said `401` again three times is neither more polite
    // nor more informative.
    struct Refusing(Arc<AtomicU32>);
    #[async_trait]
    impl Http for Refusing {
        async fn send(&self, _request: &Request) -> Result<Response, Unreachable> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(Response {
                status: 401,
                headers: Vec::new(),
                body: "no".to_owned(),
            })
        }
    }
    let asked = Arc::new(AtomicU32::new(0));
    let retrying = Retrying::around(Refusing(Arc::clone(&asked)));
    let answered = retrying.send(&request(Method::Get)).await;
    assert!(answered.is_ok_and(|response| response.status == 401));
    assert_eq!(asked.load(Ordering::SeqCst), 1);
}
