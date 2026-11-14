use std::sync::{Arc, Mutex};

use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::Router;
use lemonfiber_ports::service::{Event, Happened};

use super::Told;

pub(crate) const KEY: &str = "0123456789abcdef0123456789abcdef";

/// What a fake core heard: each bearer and body, in order.
pub(crate) type Posts = Arc<Mutex<Vec<(String, String)>>>;

/// A fake core answering `status` to each post, and what it heard.
pub(crate) fn core(status: StatusCode) -> Option<(lemonfiber_pinned::fake::Served, Posts)> {
    let posts: Posts = Arc::default();
    let heard = Arc::clone(&posts);
    let app = Router::new().route(
        "/api/notify/events",
        post(move |headers: HeaderMap, body: String| {
            let heard = Arc::clone(&heard);
            async move {
                let bearer = headers
                    .get("authorization")
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or_default()
                    .to_owned();
                if let Ok(mut heard) = heard.lock() {
                    heard.push((bearer, body));
                }
                status
            }
        }),
    );
    let served = lemonfiber_pinned::fake::tls(app);
    assert!(served.is_some(), "no fake core could be served");
    served.map(|served| (served, posts))
}

/// Where a fake core takes events.
pub(crate) fn address(served: &lemonfiber_pinned::fake::Served) -> String {
    format!("{}api/notify/events", served.base)
}

pub(crate) fn added(name: &str) -> Event {
    Event {
        event: Happened::Added,
        reference: Some(name.to_lowercase()),
        name: name.to_owned(),
        reason: None,
        cleared: false,
    }
}

#[test]
fn an_address_pin_or_key_that_will_not_do_is_refused() {
    let pin = "a".repeat(64);
    let address = "https://core.invalid/api/notify/events";
    assert!(Told::new(address, &pin, KEY).is_some());
    assert!(Told::new("http://core.invalid/api/notify/events", &pin, KEY).is_none());
    assert!(Told::new(address, &"A".repeat(64), KEY).is_none());
    assert!(Told::new(address, &"a".repeat(63), KEY).is_none());
    assert!(Told::new(address, &pin, "a key\nwith a line").is_none());
    assert!(Told::new(address, &pin, "").is_none());
}

#[tokio::test]
async fn an_event_is_posted_as_json_with_the_key_as_its_bearer() {
    let Some((served, posts)) = core(StatusCode::ACCEPTED) else {
        return;
    };
    let told = Told::new(&address(&served), &served.pin, KEY);
    let sent = match &told {
        Some(told) => told.sent(&added("Dune")).await,
        None => false,
    };
    assert!(sent);
    let heard = posts.lock().map(|posts| posts.clone()).unwrap_or_default();
    let [(bearer, body)] = heard.as_slice() else {
        return assert_eq!(heard.len(), 1);
    };
    assert_eq!(*bearer, format!("Bearer {KEY}"));
    assert_eq!(
        serde_json::from_str::<Event>(body).ok(),
        Some(added("Dune"))
    );
}

#[tokio::test]
async fn an_answer_other_than_a_success_is_not_answered() {
    let Some((served, _)) = core(StatusCode::UNAUTHORIZED) else {
        return;
    };
    let told = Told::new(&address(&served), &served.pin, KEY);
    let sent = match &told {
        Some(told) => told.sent(&added("Dune")).await,
        None => true,
    };
    assert!(!sent);
}

#[tokio::test]
async fn a_core_presenting_another_certificate_is_sent_nothing() {
    let Some((served, posts)) = core(StatusCode::ACCEPTED) else {
        return;
    };
    let told = Told::new(&address(&served), &"b".repeat(64), KEY);
    let sent = match &told {
        Some(told) => told.sent(&added("Dune")).await,
        None => true,
    };
    assert!(!sent);
    assert_eq!(posts.lock().map(|posts| posts.len()).ok(), Some(0));
}
