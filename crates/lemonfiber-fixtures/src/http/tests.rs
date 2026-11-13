use std::time::Duration;

use lemonfiber_ports::http::{Http as _, Method, Request, Response};

use super::{Answer, Fake};

/// A request of `method` to `url`, carrying no body.
fn asked(method: Method, url: &str) -> Request {
    Request {
        method,
        url: url.to_owned(),
        headers: Vec::new(),
        body: None,
        pinned: None,
    }
}

/// The status `fake` answers `method` to `url` with, or nothing where it answers nothing.
async fn status(fake: &Fake, method: Method, url: &str) -> Option<u16> {
    fake.send(&asked(method, url))
        .await
        .ok()
        .map(|answered| answered.status)
}

#[tokio::test]
async fn a_route_answers_only_the_method_it_names() {
    let fake = Fake::by_route(vec![
        (Method::Post, "/thing", Answer::reply(201, "")),
        (Method::Get, "/thing", Answer::reply(200, "[]")),
    ]);

    assert_eq!(
        status(&fake, Method::Get, "http://x/thing").await,
        Some(200)
    );
    assert_eq!(
        status(&fake, Method::Post, "http://x/thing").await,
        Some(201)
    );
    assert_eq!(status(&fake, Method::Put, "http://x/thing").await, None);
}

#[tokio::test]
async fn a_route_of_one_method_answers_in_turn_the_last_repeating() {
    let fake = Fake::by_route_in_turn(vec![(
        Method::Get,
        "/thing",
        vec![Answer::reply(404, ""), Answer::reply(200, "[]")],
    )]);

    assert_eq!(status(&fake, Method::Post, "http://x/thing").await, None);
    assert_eq!(
        status(&fake, Method::Get, "http://x/thing").await,
        Some(404)
    );
    assert_eq!(
        status(&fake, Method::Get, "http://x/thing").await,
        Some(200)
    );
    assert_eq!(
        status(&fake, Method::Get, "http://x/thing").await,
        Some(200)
    );
}

#[tokio::test(start_paused = true)]
async fn a_hanging_fake_never_answers() {
    let fake = Fake::hanging();
    let request = asked(Method::Get, "http://x/thing");
    let waited = tokio::time::timeout(Duration::from_secs(60), fake.send(&request)).await;
    assert!(waited.is_err());
    assert!(fake.asked_for("/thing"));
}

#[tokio::test]
async fn a_handing_fake_answers_by_the_function_it_was_handed() {
    let fake = Fake::handing(|request| {
        Box::pin(std::future::ready(Ok(Response {
            status: 207,
            headers: Vec::new(),
            body: request.url,
        })))
    });
    let fetched = fake.fetch(&asked(Method::Get, "http://x/thing"), 64).await;
    assert_eq!(
        fetched.map(|fetched| (fetched.status, fetched.bytes)),
        Ok((207, Some(b"http://x/thing".to_vec())))
    );
}

#[tokio::test]
async fn a_distributor_takes_octets_and_refuses_a_request_carrying_none() {
    let fake = Fake::distributing(vec![("/a", Answer::reply(201, ""))]);
    let request = asked(Method::Post, "http://x/a");
    let taken = fake.send_octets(&request, b"sealed").await;
    let refused = fake.send(&request).await;
    assert_eq!(taken.map(|answered| answered.status), Ok(201));
    assert_eq!(
        refused.err().map(|failure| failure.reason),
        Some("only octets are taken here".to_owned())
    );
    assert_eq!(fake.octets(), vec![(request, b"sealed".to_vec())]);
    assert_eq!(fake.requests().len(), 2);
}

#[tokio::test]
async fn octets_answer_a_fetch_within_its_bound_and_never_a_send() {
    let fake = Fake::always(Answer::Octets {
        status: 200,
        bytes: vec![0xff, 0xfe],
    });
    let request = asked(Method::Get, "http://x/bytes");
    let within = fake.fetch(&request, 2).await;
    let beyond = fake.fetch(&request, 1).await;
    let sent = fake.send(&request).await;
    assert_eq!(
        within.map(|fetched| fetched.bytes),
        Ok(Some(vec![0xff, 0xfe]))
    );
    assert_eq!(beyond.map(|fetched| fetched.bytes), Ok(None));
    assert!(sent.is_err());
}

#[tokio::test]
async fn a_fetch_of_nothing_answering_is_unreachable() {
    let fake = Fake::silent();
    let fetched = fake.fetch(&asked(Method::Get, "http://x/thing"), 64).await;
    assert!(fetched.is_err());
}

#[tokio::test]
async fn unreached_says_whether_a_connection_was_made() {
    let request = asked(Method::Get, "http://x/thing");
    let refused = Fake::always(Answer::unreached("connection refused"))
        .send(&request)
        .await;
    let quiet = Fake::always(Answer::went_quiet("operation timed out"))
        .send(&request)
        .await;
    assert_eq!(
        refused
            .err()
            .map(|failure| (failure.reason, failure.connected)),
        Some(("connection refused".to_owned(), false))
    );
    assert_eq!(
        quiet
            .err()
            .map(|failure| (failure.reason, failure.connected)),
        Some(("operation timed out".to_owned(), true))
    );
}
