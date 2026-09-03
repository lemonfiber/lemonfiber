//! Registering each \\*arr as an application in Prowlarr.
//!
//! The same shape again, matched by the base URL Prowlarr reaches the service on.

mod common;

use common::service::*;
use std::sync::Mutex;

use async_trait::async_trait;
use lemonfiber_core::journal::Journal;
use lemonfiber_core::ports::service::{
    AppSync, Application, ApplicationKind, Failure, RegisteredApplication,
};
use lemonfiber_core::seed::{wire_applications, State};

// ---- Prowlarr applications: the same driver, matched by base URL not label. ----

/// A Prowlarr that answers the app-sync driver from a script.
struct FakeProwlarr {
    mode: Mode,
    applications: Mutex<Vec<RegisteredApplication>>,
    reads: Mutex<u32>,
    next_id: Mutex<u32>,
}

impl FakeProwlarr {
    fn with(mode: Mode, applications: Vec<RegisteredApplication>) -> Self {
        Self {
            mode,
            applications: Mutex::new(applications),
            reads: Mutex::new(0),
            next_id: Mutex::new(100),
        }
    }
}

#[async_trait]
impl AppSync for FakeProwlarr {
    async fn register_application(&self, application: &Application) -> Result<(), Failure> {
        match self.mode {
            Mode::Down => Err(down("prowlarr")),
            Mode::RejectsRegister => Err(Failure::Refused {
                service: "prowlarr".to_owned(),
                detail: "HTTP 400: unknown implementation".to_owned(),
            }),
            Mode::Swallows => Ok(()),
            _ => {
                if let (Ok(mut applications), Ok(mut id)) =
                    (self.applications.lock(), self.next_id.lock())
                {
                    applications.push(RegisteredApplication {
                        id: id.to_string(),
                        // Prowlarr stores the canonical address, dropping a
                        // trailing slash — as its `fields` read back.
                        base_url: application.base_url.trim_end_matches('/').to_owned(),
                    });
                    *id += 1;
                }
                Ok(())
            }
        }
    }

    async fn applications(&self) -> Result<Vec<RegisteredApplication>, Failure> {
        let count = match self.reads.lock() {
            Ok(mut reads) => {
                *reads += 1;
                *reads
            }
            Err(_) => 0,
        };
        match self.mode {
            Mode::Down => Err(down("prowlarr")),
            Mode::RefusesList => Err(Failure::Unauthorised {
                service: "prowlarr".to_owned(),
            }),
            Mode::DropsAfterRegister if count >= 2 => Err(down("prowlarr")),
            _ => Ok(self
                .applications
                .lock()
                .map(|applications| applications.clone())
                .unwrap_or_default()),
        }
    }
}

fn app(base_url: &str) -> Application {
    Application {
        name: "Sonarr".to_owned(),
        kind: ApplicationKind::Sonarr,
        prowlarr_url: "http://prowlarr:9696".to_owned(),
        base_url: base_url.to_owned(),
        api_key: "arr-key".to_owned(),
    }
}

/// Run the app-sync driver for the wanted applications, returning their states
/// and the number of changes journalled.
async fn seed_applications(prowlarr: FakeProwlarr, wanted: &[Application]) -> (Vec<State>, usize) {
    let mut journal = Journal::new();
    let wirings = wire_applications(&prowlarr, "prowlarr", wanted, &mut journal, "t").await;
    let states = wirings.into_iter().map(|wiring| wiring.state).collect();
    (states, journal.changes().len())
}

#[tokio::test]
async fn an_absent_application_is_registered_read_back_and_recorded() {
    let (states, recorded) = seed_applications(
        FakeProwlarr::with(Mode::Normal, Vec::new()),
        &[app("http://sonarr:8989")],
    )
    .await;
    assert_eq!(states, vec![State::Wired]);
    assert_eq!(recorded, 1, "the write is journalled so it can be undone");
}

#[tokio::test]
async fn an_application_at_the_same_base_url_is_left_untouched_despite_a_different_name() {
    // Identity is the address Prowlarr reaches, not the label: the operator
    // renamed the application, but it points at the same *arr, so it is left
    // alone rather than registered a second time.
    let existing = vec![RegisteredApplication {
        id: "1".to_owned(),
        base_url: "http://sonarr:8989".to_owned(),
    }];
    let mut renamed = app("http://sonarr:8989");
    renamed.name = "Sonarr — my own name".to_owned();
    let (states, recorded) =
        seed_applications(FakeProwlarr::with(Mode::Normal, existing), &[renamed]).await;
    assert_eq!(states, vec![State::AlreadyWired]);
    assert_eq!(
        recorded, 0,
        "an application already at the address is not duplicated"
    );
}

#[tokio::test]
async fn a_present_application_is_matched_despite_a_trailing_slash() {
    // Idempotent across the same normalization: Prowlarr holds the canonical
    // address, and a wanted one that differs only by a trailing slash is left
    // alone, not re-registered.
    let existing = vec![RegisteredApplication {
        id: "1".to_owned(),
        base_url: "http://sonarr:8989".to_owned(),
    }];
    let (states, recorded) = seed_applications(
        FakeProwlarr::with(Mode::Normal, existing),
        &[app("http://sonarr:8989/")],
    )
    .await;
    assert_eq!(states, vec![State::AlreadyWired]);
    assert_eq!(recorded, 0);
}

#[tokio::test]
async fn an_unavailable_prowlarr_skips_every_application() {
    let (states, recorded) = seed_applications(
        FakeProwlarr::with(Mode::Down, Vec::new()),
        &[app("http://sonarr:8989"), app("http://radarr:7878")],
    )
    .await;
    // Counted before it is judged: `all` over an empty list is true, so a run that
    // produced no state at all would pass a test named for skipping every application
    // while proving nothing was skipped.
    assert_eq!(states.len(), 2, "{states:?}");
    assert!(
        states
            .iter()
            .all(|state| matches!(state, State::Skipped { .. })),
        "{states:?}"
    );
    assert_eq!(recorded, 0);
}

#[tokio::test]
async fn a_prowlarr_that_refuses_the_application_listing_fails() {
    let (states, _) = seed_applications(
        FakeProwlarr::with(Mode::RefusesList, Vec::new()),
        &[app("http://sonarr:8989")],
    )
    .await;
    assert!(
        matches!(states.as_slice(), [State::Failed { .. }]),
        "{states:?}"
    );
}

#[tokio::test]
async fn a_rejected_application_registration_fails_with_prowlarrs_own_words() {
    let (states, recorded) = seed_applications(
        FakeProwlarr::with(Mode::RejectsRegister, Vec::new()),
        &[app("http://sonarr:8989")],
    )
    .await;
    let detail = match states.as_slice() {
        [State::Failed { detail }] => Some(detail.clone()),
        _ => None,
    };
    assert!(
        detail.is_some_and(|words| words.contains("unknown implementation")),
        "the service's own words survive: {states:?}"
    );
    assert_eq!(recorded, 0, "a rejected write is not journalled");
}

#[tokio::test]
async fn an_application_write_that_does_not_appear_when_read_back_is_a_failure() {
    // Accepted but not reported back, so it did not land — not done, not recorded.
    let (states, recorded) = seed_applications(
        FakeProwlarr::with(Mode::Swallows, Vec::new()),
        &[app("http://sonarr:8989")],
    )
    .await;
    assert!(
        matches!(states.as_slice(), [State::Failed { .. }]),
        "{states:?}"
    );
    assert_eq!(recorded, 0);
}

#[tokio::test]
async fn a_prowlarr_that_stops_answering_after_the_write_is_skipped() {
    // The write went out but could not be confirmed, so it is left for a later
    // run to reconcile rather than declared wired.
    let (states, recorded) = seed_applications(
        FakeProwlarr::with(Mode::DropsAfterRegister, Vec::new()),
        &[app("http://sonarr:8989")],
    )
    .await;
    assert!(
        matches!(states.as_slice(), [State::Skipped { .. }]),
        "{states:?}"
    );
    assert_eq!(recorded, 0, "an unconfirmed write is not recorded as done");
}
