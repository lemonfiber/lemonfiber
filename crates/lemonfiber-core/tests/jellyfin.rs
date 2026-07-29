//! The Jellyfin setup client, driven through the HTTP port against a fake
//! transport.
//!
//! Driving the first-run wizard is more than one call — create the account, then
//! finish setup — so the fake answers from a queue and remembers every request,
//! and each test scripts exactly the sequence a branch needs with nothing running.
//! The client speaks an async trait built on another, so it is driven from here
//! rather than in-crate, where it would be compiled twice.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lemonfiber_core::jellyfin::Jellyfin;
use lemonfiber_core::ports::http::{Http, Request, Response, Unreachable};
use lemonfiber_core::ports::service::{Failure, MediaServer};

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
            }),
        }
    }
}

fn jellyfin(fake: &Arc<Fake>) -> Jellyfin {
    let http: Arc<dyn Http> = fake.clone();
    Jellyfin::new(http, "http://127.0.0.1:8096", "jellyfin")
}

#[tokio::test]
async fn a_completed_wizard_is_reported() {
    let fake = Fake::replying(vec![(200, r#"{"StartupWizardCompleted":true}"#)]);
    assert_eq!(jellyfin(&fake).startup_completed().await.ok(), Some(true));
    assert!(fake
        .requests()
        .first()
        .is_some_and(|request| request.url.ends_with("/System/Info/Public")));
}

#[tokio::test]
async fn an_incomplete_or_unstated_wizard_reads_as_not_done() {
    let fake = Fake::replying(vec![(200, r#"{"StartupWizardCompleted":false}"#)]);
    assert_eq!(jellyfin(&fake).startup_completed().await.ok(), Some(false));
    // A response that omits the field is a server too fresh to have set it: not
    // done, the same as false.
    let bare = Fake::replying(vec![(200, "{}")]);
    assert_eq!(jellyfin(&bare).startup_completed().await.ok(), Some(false));
}

#[tokio::test]
async fn an_unreadable_public_info_is_refused() {
    let fake = Fake::replying(vec![(200, "not json")]);
    assert!(matches!(
        jellyfin(&fake).startup_completed().await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn a_refused_public_info_carries_the_status() {
    let fake = Fake::replying(vec![(503, "")]);
    assert!(matches!(
        jellyfin(&fake).startup_completed().await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn an_unreachable_jellyfin_is_unavailable() {
    let fake = Fake::silent();
    assert!(matches!(
        jellyfin(&fake).startup_completed().await,
        Err(Failure::Unavailable { .. })
    ));
}

#[tokio::test]
async fn creating_the_admin_posts_the_account_then_completes_setup() {
    let fake = Fake::replying(vec![(200, ""), (200, "")]);
    assert!(jellyfin(&fake)
        .create_admin("admin", "secret")
        .await
        .is_ok());

    let requests = fake.requests();
    let first = requests.first();
    assert!(first.is_some_and(|request| request.url.ends_with("/Startup/User")));
    let body = first
        .and_then(|request| request.body.clone())
        .unwrap_or_default();
    assert!(body.contains(r#""Name":"admin""#), "{body}");
    assert!(body.contains(r#""Password":"secret""#), "{body}");
    // Setup is finished only after the account is made.
    assert!(requests
        .get(1)
        .is_some_and(|request| request.url.ends_with("/Startup/Complete")));
}

#[tokio::test]
async fn a_rejected_admin_creation_is_refused_and_setup_is_not_finished() {
    let fake = Fake::replying(vec![(400, "user already exists")]);
    assert!(matches!(
        jellyfin(&fake).create_admin("admin", "secret").await,
        Err(Failure::Refused { .. })
    ));
    // Only the failed create was attempted; completion was never reached.
    assert_eq!(fake.requests().len(), 1);
}

#[tokio::test]
async fn a_rejected_completion_is_refused() {
    let fake = Fake::replying(vec![(200, ""), (500, "boom")]);
    assert!(matches!(
        jellyfin(&fake).create_admin("admin", "secret").await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn creating_the_admin_on_an_unreachable_jellyfin_is_unavailable() {
    let fake = Fake::silent();
    assert!(matches!(
        jellyfin(&fake).create_admin("admin", "secret").await,
        Err(Failure::Unavailable { .. })
    ));
}
