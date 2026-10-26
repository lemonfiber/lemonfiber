use lemonfiber_ports::http::{Http as _, Method, Request};

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
