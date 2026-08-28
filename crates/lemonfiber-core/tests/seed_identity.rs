//! Pointing Seerr at Jellyfin, with a credential minted for it.
//!
//! Two services and a secret, and the one case that must never be overwritten: a
//! Seerr the household already set up is left as they left it.

mod common;

use common::service::*;
use std::sync::Mutex;

use async_trait::async_trait;
use lemonfiber_core::ports::service::{Failure, HouseholdRequest, MediaServer, Requests, Telling};
use lemonfiber_core::seed::{wire_jellyfin_identity, State};

// ---- Jellyfin as Seerr's identity: two services and a minted credential. ----

/// How Jellyfin's setup answers.
enum Startup {
    /// The wizard has already run.
    Completed,
    /// The wizard has not run.
    Fresh,
    /// Not answering.
    Down,
}

/// How Jellyfin answers the account creation. A refusal stands for every
/// non-success; the not-answering path is exercised through the startup read.
enum Create {
    Ok,
    Rejects,
}

struct FakeMedia {
    startup: Startup,
    create: Create,
}

#[async_trait]
impl MediaServer for FakeMedia {
    async fn startup_completed(&self) -> Result<bool, Failure> {
        match self.startup {
            Startup::Completed => Ok(true),
            Startup::Fresh => Ok(false),
            Startup::Down => Err(down("jellyfin")),
        }
    }

    async fn create_admin(&self, _name: &str, _password: &str) -> Result<(), Failure> {
        match self.create {
            Create::Ok => Ok(()),
            Create::Rejects => Err(Failure::Refused {
                service: "jellyfin".to_owned(),
                detail: "HTTP 400: user already exists".to_owned(),
            }),
        }
    }
}

/// How Seerr answers a read of its initialised state.
enum Init {
    /// Not initialised.
    Fresh,
    /// Already initialised.
    Done,
    /// Not answering.
    Down,
}

/// How Seerr answers the sign-in. A refusal stands for every non-success; the
/// not-answering path is exercised through the initialised read.
enum Configure {
    Ok,
    Rejects,
}

struct FakeReq {
    /// The state reported on the first read (the gate) and on the second (the
    /// read-back), so a fresh-then-confirmed sequence can be scripted.
    gate: Init,
    readback: Init,
    configure: Configure,
    calls: Mutex<u32>,
    /// What the service is holding for what the household gets told, which `tell`
    /// overwrites so a test can read back what would have reached it.
    told: Mutex<Telling>,
}

impl FakeReq {
    fn new(gate: Init, readback: Init, configure: Configure) -> Self {
        Self {
            gate,
            readback,
            configure,
            calls: Mutex::new(0),
            told: Mutex::new(Telling::default()),
        }
    }

    fn reading(state: &Init) -> Result<bool, Failure> {
        match state {
            Init::Fresh => Ok(false),
            Init::Done => Ok(true),
            Init::Down => Err(down("seerr")),
        }
    }
}

#[async_trait]
impl Requests for FakeReq {
    async fn initialized(&self) -> Result<bool, Failure> {
        let count = match self.calls.lock() {
            Ok(mut calls) => {
                *calls += 1;
                *calls
            }
            Err(_) => 0,
        };
        if count <= 1 {
            Self::reading(&self.gate)
        } else {
            Self::reading(&self.readback)
        }
    }

    /// Signing in is the first half of configuring identity, and fails the same way.
    async fn sign_in(
        &self,
        username: &str,
        password: &str,
        server_url: &str,
    ) -> Result<(), Failure> {
        self.configure_identity(username, password, server_url)
            .await
    }

    /// The seeding driver never reads the household's requests; the household view is
    /// driven from its own tests.
    async fn requests(&self) -> Result<Vec<HouseholdRequest>, Failure> {
        Ok(Vec::new())
    }

    /// This fake drives the identity wiring, which asks about no fulfilment target.
    async fn fulfilment_targets(
        &self,
    ) -> Result<Vec<lemonfiber_core::ports::service::RegisteredTarget>, Failure> {
        Ok(Vec::new())
    }

    async fn add_fulfilment_target(
        &self,
        _target: &lemonfiber_core::ports::service::FulfilmentTarget,
    ) -> Result<(), Failure> {
        Ok(())
    }

    /// What this fake was told to say the service is telling the household.
    async fn telling(&self) -> Result<Telling, Failure> {
        match self.gate {
            Init::Down => Err(down("seerr")),
            Init::Fresh | Init::Done => Ok(*self
                .told
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)),
        }
    }

    /// Records what was asked for, so a test reads what would reach the service.
    async fn tell(&self, telling: &Telling) -> Result<(), Failure> {
        match self.gate {
            Init::Down => Err(down("seerr")),
            Init::Fresh | Init::Done => {
                if let Ok(mut told) = self.told.lock() {
                    *told = *telling;
                }
                Ok(())
            }
        }
    }

    async fn configure_identity(
        &self,
        _username: &str,
        _password: &str,
        _server_url: &str,
    ) -> Result<(), Failure> {
        match self.configure {
            Configure::Ok => Ok(()),
            Configure::Rejects => Err(Failure::Refused {
                service: "seerr".to_owned(),
                detail: "HTTP 500: credentials rejected".to_owned(),
            }),
        }
    }
}

/// Run the identity driver, returning the resulting state and the password to
/// record (present only when the account was newly minted).
async fn identity(
    media: FakeMedia,
    seerr: FakeReq,
    random: Option<Vec<u8>>,
    recorded: Option<&str>,
) -> (State, Option<String>) {
    let random = lemonfiber_fixtures::ports::Chance::exactly(random);
    let (wiring, minted) =
        wire_jellyfin_identity(&media, &seerr, &random, recorded, "http://jellyfin:8096").await;
    (wiring.state, minted)
}

fn media(startup: Startup, create: Create) -> FakeMedia {
    FakeMedia { startup, create }
}

const RANDOM: [u8; 24] = [0x11; 24];

#[tokio::test]
async fn a_jellyfin_that_is_not_answering_skips_the_identity() {
    let (state, minted) = identity(
        media(Startup::Down, Create::Ok),
        FakeReq::new(Init::Fresh, Init::Done, Configure::Ok),
        Some(RANDOM.to_vec()),
        None,
    )
    .await;
    assert!(matches!(state, State::Skipped { .. }), "{state:?}");
    assert!(minted.is_none(), "nothing was minted");
}

#[tokio::test]
async fn no_randomness_fails_rather_than_setting_a_guessable_password() {
    let (state, minted) = identity(
        media(Startup::Fresh, Create::Ok),
        FakeReq::new(Init::Fresh, Init::Done, Configure::Ok),
        None,
        None,
    )
    .await;
    assert!(matches!(state, State::Failed { .. }), "{state:?}");
    assert!(minted.is_none());
}

#[tokio::test]
async fn a_fresh_stack_mints_the_admin_and_wires_the_identity() {
    let (state, minted) = identity(
        media(Startup::Fresh, Create::Ok),
        FakeReq::new(Init::Fresh, Init::Done, Configure::Ok),
        Some(RANDOM.to_vec()),
        None,
    )
    .await;
    assert_eq!(state, State::Wired);
    assert!(
        minted.is_some(),
        "the minted password is handed back to record"
    );
}

#[tokio::test]
async fn a_rejected_admin_creation_fails_with_the_services_own_words() {
    let (state, minted) = identity(
        media(Startup::Fresh, Create::Rejects),
        FakeReq::new(Init::Fresh, Init::Done, Configure::Ok),
        Some(RANDOM.to_vec()),
        None,
    )
    .await;
    assert!(matches!(state, State::Failed { .. }), "{state:?}");
    assert!(minted.is_none(), "a failed creation records no password");
}

#[tokio::test]
async fn a_jellyfin_set_up_outside_lemonfiber_is_skipped() {
    // The wizard has run but lemonfiber recorded no password, so the household set
    // it up: its credential is unknown, and the identity cannot be wired.
    let (state, minted) = identity(
        media(Startup::Completed, Create::Ok),
        FakeReq::new(Init::Fresh, Init::Done, Configure::Ok),
        Some(RANDOM.to_vec()),
        None,
    )
    .await;
    assert!(matches!(state, State::Skipped { .. }), "{state:?}");
    assert!(minted.is_none());
}

#[tokio::test]
async fn a_recorded_password_wires_an_already_initialised_seerr() {
    // Jellyfin was set up by lemonfiber before (password recorded) and Seerr is
    // already initialised: nothing is minted and nothing re-pointed.
    let (state, minted) = identity(
        media(Startup::Completed, Create::Ok),
        FakeReq::new(Init::Done, Init::Done, Configure::Ok),
        None,
        Some("minted-earlier"),
    )
    .await;
    assert_eq!(state, State::AlreadyWired);
    assert!(minted.is_none(), "an idempotent run mints nothing");
}

#[tokio::test]
async fn a_seerr_that_is_not_answering_still_records_the_minted_password() {
    // Jellyfin's account was created this run, so its password must be recorded
    // even though Seerr could not then be reached to finish the wiring.
    let (state, minted) = identity(
        media(Startup::Fresh, Create::Ok),
        FakeReq::new(Init::Down, Init::Done, Configure::Ok),
        Some(RANDOM.to_vec()),
        None,
    )
    .await;
    assert!(matches!(state, State::Skipped { .. }), "{state:?}");
    assert!(
        minted.is_some(),
        "the account holds the minted password, so it is recorded regardless"
    );
}

#[tokio::test]
async fn a_rejected_sign_in_fails() {
    let (state, _) = identity(
        media(Startup::Fresh, Create::Ok),
        FakeReq::new(Init::Fresh, Init::Done, Configure::Rejects),
        Some(RANDOM.to_vec()),
        None,
    )
    .await;
    assert!(matches!(state, State::Failed { .. }), "{state:?}");
}

#[tokio::test]
async fn a_sign_in_that_does_not_take_is_a_failure() {
    // Seerr accepted the sign-in but still reports itself uninitialised on the
    // read-back, so the write did not land.
    let (state, _) = identity(
        media(Startup::Fresh, Create::Ok),
        FakeReq::new(Init::Fresh, Init::Fresh, Configure::Ok),
        Some(RANDOM.to_vec()),
        None,
    )
    .await;
    assert!(matches!(state, State::Failed { .. }), "{state:?}");
}

#[tokio::test]
async fn a_read_back_that_cannot_be_reached_is_skipped() {
    let (state, _) = identity(
        media(Startup::Fresh, Create::Ok),
        FakeReq::new(Init::Fresh, Init::Down, Configure::Ok),
        Some(RANDOM.to_vec()),
        None,
    )
    .await;
    assert!(matches!(state, State::Skipped { .. }), "{state:?}");
}
