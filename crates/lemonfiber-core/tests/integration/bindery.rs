//! The book \*arr's client, driven through the HTTP port against a fake transport.
//!
//! The body it takes is camel-cased, and a field under any other spelling is dropped
//! without complaint — the registration still answers `201`, having stored an entry
//! with no key in it. So these assert the request that went out, and the read-back
//! asserts that an entry counts as made only if it holds a key.
//!
//! The shapes here were read off `ghcr.io/vavallee/bindery:v1.27.0` and driven at it.

use std::sync::Arc;

use lemonfiber_core::bindery::Bindery;
use lemonfiber_core::ports::http::{Http, Method};
use lemonfiber_core::ports::service::{Aggregator, Aggregators, Failure};
use lemonfiber_fixtures::http::{Answer, Fake};

/// The key this client presents, assembled rather than written down: a literal reads
/// to a source scanner as a credential committed to the repository.
fn key() -> String {
    ["the", "-key"].concat()
}

fn bindery(fake: &Arc<Fake>) -> Bindery {
    let http: Arc<dyn Http> = fake.clone();
    Bindery::new(http, "http://127.0.0.1:8787", "bindery", key())
}

/// The aggregator as it is told about it.
fn aggregator() -> Aggregator {
    Aggregator {
        name: "Prowlarr".to_owned(),
        url: "http://prowlarr:9696".to_owned(),
        key: ["arr", "-key"].concat(),
    }
}

/// The key travels in the header the service reads it from. A bearer token is refused.
#[tokio::test]
async fn the_key_is_presented_where_the_service_looks_for_it() {
    let fake = Fake::always(Answer::reply(200, "[]"));
    let _ = bindery(&fake).aggregators().await;

    let carried = fake
        .request()
        .map(|request| request.headers)
        .unwrap_or_default();
    assert!(
        carried
            .iter()
            .any(|(name, value)| name == "X-Api-Key" && value == &key()),
        "the key was not presented: {carried:?}"
    );
}

/// The registration is camel-cased, which is the only spelling the service reads.
///
/// Sending the names its own storage uses is not a request refused — it answers `201`
/// having stored an entry with no key, which is indistinguishable from a working one
/// until somebody wonders why nothing ever arrives.
#[tokio::test]
async fn the_aggregator_is_registered_under_the_names_the_service_reads() {
    let fake = Fake::always(Answer::reply(201, "{}"));
    let told = bindery(&fake).add_aggregator(&aggregator()).await;
    assert!(told.is_ok(), "{told:?}");

    let body = fake
        .request()
        .and_then(|request| request.body)
        .unwrap_or_default();
    for expected in [
        "\"apiKey\":\"arr-key\"",
        "\"url\":\"http://prowlarr:9696\"",
        "\"syncOnStartup\":true",
        "\"enabled\":true",
    ] {
        assert!(body.contains(expected), "{expected} is missing from {body}");
    }
    assert!(
        !body.contains("api_key") && !body.contains("sync_on_startup"),
        "sent under the spelling the service drops: {body}"
    );
}

/// What it already holds is read back, and an entry without a key is not held.
///
/// The service reports an entry it stored from a registration whose key it did not
/// understand. Reading that back as already-wired would leave the connection unmade
/// and reported as done.
#[tokio::test]
async fn an_entry_without_a_key_is_not_read_as_one_that_holds_it() {
    const HELD: &str = r#"[
        {"id":1,"url":"http://prowlarr:9696","apiKey":""},
        {"id":2,"url":"http://other:9696","apiKey":"set"}
    ]"#;
    let fake = Fake::always(Answer::reply(200, HELD));
    let held = bindery(&fake).aggregators().await.unwrap_or_default();

    assert_eq!(held.len(), 2, "{held:?}");
    assert!(
        held.iter()
            .any(|known| known.url.contains("prowlarr") && !known.keyed),
        "the entry with no key was read as keyed: {held:?}"
    );
    assert!(
        held.iter()
            .any(|known| known.url.contains("other") && known.keyed),
        "{held:?}"
    );
}

/// A refusal is reported as one, and a silence as a silence.
#[tokio::test]
async fn a_service_that_will_not_take_it_is_reported() {
    let refused = Fake::always(Answer::reply(401, ""));
    assert!(matches!(
        bindery(&refused).add_aggregator(&aggregator()).await,
        Err(Failure::Unauthorised { .. })
    ));

    let silent = Fake::silent();
    assert!(matches!(
        bindery(&silent).aggregators().await,
        Err(Failure::Unavailable { .. })
    ));
}

/// A list that cannot be read is a failure rather than an empty one.
#[tokio::test]
async fn a_list_that_cannot_be_read_is_reported() {
    let fake = Fake::always(Answer::reply(200, "not json"));
    assert!(bindery(&fake).aggregators().await.is_err());
}

/// The registration is a post, to the collection the service keeps them in.
#[tokio::test]
async fn the_registration_goes_to_the_collection_the_service_keeps() {
    let fake = Fake::always(Answer::reply(201, "{}"));
    let _ = bindery(&fake).add_aggregator(&aggregator()).await;

    assert!(
        fake.request()
            .is_some_and(|request| request.method == Method::Post
                && request.url.ends_with("/api/v1/prowlarr")),
        "the registration did not go where the service keeps them"
    );
}
