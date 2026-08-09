//! The Seerr identity client, driven through the HTTP port against a fake
//! transport.
//!
//! Configuring identity is more than one call — sign in through Jellyfin, then
//! finish setup — so the fake answers from a queue and remembers every request,
//! and each test scripts exactly the sequence a branch needs with nothing running.
//! The client speaks an async trait built on another, so it is driven from here
//! rather than in-crate.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lemonfiber_core::ports::http::{Http, Request, Response, Unreachable};
use lemonfiber_core::ports::service::{Failure, Requests};
use lemonfiber_core::seerr::Seerr;

/// A transport that answers from a queue and remembers every request.
struct Fake {
    replies: Mutex<VecDeque<(u16, &'static str)>>,
    seen: Mutex<Vec<Request>>,
    silent: bool,
}

impl Fake {
    fn replying(replies: Vec<(u16, &'static str)>) -> Arc<Self> {
        Arc::new(Self {
            replies: Mutex::new(replies.into()),
            seen: Mutex::new(Vec::new()),
            silent: false,
        })
    }

    fn silent() -> Arc<Self> {
        Arc::new(Self {
            replies: Mutex::new(VecDeque::new()),
            seen: Mutex::new(Vec::new()),
            silent: true,
        })
    }

    fn requests(&self) -> Vec<Request> {
        self.seen
            .lock()
            .map(|seen| seen.clone())
            .unwrap_or_default()
    }
}

#[async_trait]
impl Http for Fake {
    async fn send(&self, request: &Request) -> Result<Response, Unreachable> {
        if let Ok(mut seen) = self.seen.lock() {
            seen.push(request.clone());
        }
        if self.silent {
            return Err(Unreachable {
                url: request.url.clone(),
                reason: "connection refused".to_owned(),
                attempts: 1,
            });
        }
        match self
            .replies
            .lock()
            .ok()
            .and_then(|mut replies| replies.pop_front())
        {
            Some((status, body)) => Ok(Response {
                status,
                body: body.to_owned(),
            }),
            None => Err(Unreachable {
                url: request.url.clone(),
                reason: "nothing scripted".to_owned(),
                attempts: 1,
            }),
        }
    }
}

fn seerr(fake: &Arc<Fake>) -> Seerr {
    let http: Arc<dyn Http> = fake.clone();
    Seerr::new(http, "http://127.0.0.1:5055", "seerr")
}

/// Configure identity through the fake, for the common arguments.
async fn configure(fake: &Arc<Fake>) -> Result<(), Failure> {
    seerr(fake)
        .configure_identity("admin", "secret", "http://jellyfin:8096")
        .await
}

#[tokio::test]
async fn an_initialised_seerr_is_reported() {
    let fake = Fake::replying(vec![(200, r#"{"initialized":true}"#)]);
    assert_eq!(seerr(&fake).initialized().await.ok(), Some(true));
    assert!(fake
        .requests()
        .first()
        .is_some_and(|request| request.url.ends_with("/api/v1/settings/public")));
}

#[tokio::test]
async fn an_uninitialised_or_unstated_seerr_reads_as_not_done() {
    let fake = Fake::replying(vec![(200, r#"{"initialized":false}"#)]);
    assert_eq!(seerr(&fake).initialized().await.ok(), Some(false));
    // A response that omits the field is a Seerr too fresh to have set it.
    let bare = Fake::replying(vec![(200, "{}")]);
    assert_eq!(seerr(&bare).initialized().await.ok(), Some(false));
}

#[tokio::test]
async fn an_unreadable_public_settings_is_refused() {
    let fake = Fake::replying(vec![(200, "not json")]);
    assert!(matches!(
        seerr(&fake).initialized().await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn an_unreachable_seerr_is_unavailable_on_the_read() {
    let fake = Fake::silent();
    assert!(matches!(
        seerr(&fake).initialized().await,
        Err(Failure::Unavailable { .. })
    ));
}

#[tokio::test]
async fn configuring_identity_signs_in_through_jellyfin_then_finishes_setup() {
    let fake = Fake::replying(vec![(200, ""), (204, "")]);
    assert!(configure(&fake).await.is_ok());

    let requests = fake.requests();
    // The sign-in comes first, carrying the Jellyfin credentials as JSON.
    let first = requests.first();
    assert!(first.is_some_and(|request| request.url.ends_with("/api/v1/auth/jellyfin")));
    assert!(first.is_some_and(|request| request
        .headers
        .iter()
        .any(|(name, value)| name == "Content-Type" && value == "application/json")));
    let body = first
        .and_then(|request| request.body.clone())
        .unwrap_or_default();
    for expected in [
        r#""username":"admin""#,
        r#""password":"secret""#,
        r#""hostname":"http://jellyfin:8096""#,
        r#""email":"admin@lemonfiber.local""#,
        r#""serverType":2"#,
    ] {
        assert!(
            body.contains(expected),
            "sign-in body missing {expected}: {body}"
        );
    }
    // Setup is finished only after the sign-in.
    assert!(requests
        .get(1)
        .is_some_and(|request| request.url.ends_with("/api/v1/settings/initialize")));
}

#[tokio::test]
async fn a_rejected_sign_in_is_refused_and_setup_is_not_finished() {
    let fake = Fake::replying(vec![(500, "credentials rejected")]);
    assert!(matches!(
        configure(&fake).await,
        Err(Failure::Refused { .. })
    ));
    // Only the failed sign-in was attempted; finishing was never reached.
    assert_eq!(fake.requests().len(), 1);
}

#[tokio::test]
async fn a_rejected_finish_is_refused() {
    let fake = Fake::replying(vec![(200, ""), (500, "boom")]);
    assert!(matches!(
        configure(&fake).await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn an_unreachable_seerr_is_unavailable_on_the_sign_in() {
    let fake = Fake::silent();
    assert!(matches!(
        configure(&fake).await,
        Err(Failure::Unavailable { .. })
    ));
}

#[tokio::test]
async fn signing_in_opens_a_session_without_finishing_setup() {
    // The read path signs in only: finishing setup is somebody else's business, and a
    // read must never complete a household's configuration as a side effect.
    let fake = Fake::replying(vec![(200, "")]);
    assert!(seerr(&fake)
        .sign_in("admin", "secret", "http://jellyfin:8096")
        .await
        .is_ok());
    let requests = fake.requests();
    assert_eq!(requests.len(), 1);
    assert!(requests
        .first()
        .is_some_and(|request| request.url.ends_with("/api/v1/auth/jellyfin")));
}

#[tokio::test]
async fn the_households_requests_are_read_with_who_asked_and_what_became_of_each() {
    let page = r#"{
        "pageInfo":{"pages":1,"page":1,"results":3,"pageSize":100},
        "results":[
            {"status":2,"type":"tv","media":{"status":4,"externalServiceId":11},
             "requestedBy":{"displayName":"Alex"}},
            {"status":1,"type":"movie","media":{"status":1},
             "requestedBy":{"displayName":"Sam"}},
            {"status":5,"type":"movie","media":{"status":5,"externalServiceId":7},
             "requestedBy":{"displayName":"Alex"}}
        ]
    }"#;
    let fake = Fake::replying(vec![(200, page)]);
    let requests = seerr(&fake).requests().await.unwrap_or_default();
    let read: Vec<(&str, bool, Option<i64>, u8, u8)> = requests
        .iter()
        .map(|request| {
            (
                request.member.as_str(),
                request.kind.is_some(),
                request.item,
                request.request_status,
                request.media_status,
            )
        })
        .collect();
    // The two statuses are carried as the service's own numbers; a request no service
    // holds yet names no item, which is what leaves it with no title to find.
    assert_eq!(
        read,
        vec![
            ("Alex", true, Some(11), 2, 4),
            ("Sam", true, None, 1, 1),
            ("Alex", true, Some(7), 5, 5),
        ]
    );
    // Read newest first, so a household with more than the horizon keeps the requests
    // still worth asking about.
    assert!(fake
        .requests()
        .first()
        .is_some_and(|request| request.url.contains("sortDirection=desc")));
}

#[tokio::test]
async fn a_media_type_this_build_does_not_know_names_no_service() {
    let page = r#"{"pageInfo":{"results":1},"results":[
        {"status":2,"type":"music","media":{"status":3},"requestedBy":{"displayName":"Sam"}}
    ]}"#;
    let fake = Fake::replying(vec![(200, page)]);
    let requests = seerr(&fake).requests().await.unwrap_or_default();
    // Reported as a request whose kind is unknown rather than guessed into one of the
    // two this build files.
    assert_eq!(requests.first().map(|request| request.kind), Some(None));
}

#[tokio::test]
async fn the_requests_walk_past_the_first_page() {
    // A full first page and a total beyond it: a household with more requests than one
    // page would otherwise report only its newest.
    let filler = vec![
        r#"{"status":2,"type":"movie","media":{"status":5},"requestedBy":{"displayName":"Alex"}}"#;
        100
    ]
    .join(",");
    let page_one = Box::leak(
        format!(r#"{{"pageInfo":{{"results":101}},"results":[{filler}]}}"#).into_boxed_str(),
    );
    let page_two = r#"{"pageInfo":{"results":101},"results":[
        {"status":2,"type":"tv","media":{"status":5},"requestedBy":{"displayName":"Sam"}}
    ]}"#;
    let fake = Fake::replying(vec![(200, page_one), (200, page_two)]);
    let requests = seerr(&fake).requests().await.unwrap_or_default();
    assert_eq!(requests.len(), 101);
    assert!(fake
        .requests()
        .iter()
        .any(|request| request.url.contains("skip=100")));
}

#[tokio::test]
async fn an_unreadable_request_record_is_a_failure() {
    let fake = Fake::replying(vec![(200, "not json")]);
    assert!(seerr(&fake).requests().await.is_err());
}
