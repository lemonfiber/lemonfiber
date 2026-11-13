//! Pointing the request service at the media server, with a credential minted for it.
//!
//! Two services and a secret, and the one case that must never be overwritten: a
//! A request service the household already set up is left as they left it.

use std::sync::Mutex;

use lemonfiber_core::ports::service::Protocol;
use lemonfiber_core::seed::{wire_media_server_admin, wire_request_identity, State};
use lemonfiber_fixtures::identity::{
    Configure, Create, Init, MediaSetup, RequestSetup, Startup, NO_OWNER,
};

/// Run the identity driver, returning the resulting state and the password it
/// recorded (present only where one was newly minted).
async fn identity(
    media: MediaSetup,
    request_service: RequestSetup,
    random: Option<Vec<u8>>,
    recorded: Option<&str>,
) -> (State, Option<String>) {
    identity_keeping(media, request_service, random, recorded, true).await
}

/// The same, against a record that takes what it is given or refuses everything.
async fn identity_keeping(
    media: MediaSetup,
    request_service: RequestSetup,
    random: Option<Vec<u8>>,
    recorded: Option<&str>,
    takes: bool,
) -> (State, Option<String>) {
    let random = lemonfiber_fixtures::ports::Chance::exactly(random);
    let kept = Mutex::new(None);
    let keep = |password: &str| {
        if !takes {
            return Err("the disk is full".to_owned());
        }
        if let Ok(mut kept) = kept.lock() {
            *kept = Some(password.to_owned());
        }
        Ok(())
    };
    let state = match wire_media_server_admin(&media, &random, recorded, false, &keep).await {
        Ok(password) => {
            wire_request_identity(
                &request_service,
                Protocol("jellyfin".to_owned()),
                &password,
                "http://jellyfin:8096",
                false,
            )
            .await
            .state
        }
        Err(state) => state,
    };
    (state, kept.lock().ok().and_then(|kept| kept.clone()))
}

/// The same two services, asked what the pass would do rather than asked to do it.
async fn would_identity(
    media: MediaSetup,
    request_service: RequestSetup,
    recorded: Option<&str>,
) -> (State, Option<String>) {
    // Randomness is available on purpose: what proves nothing was minted is that
    // nothing came back, not that nothing could have.
    let random = lemonfiber_fixtures::ports::Chance::exactly(Some(RANDOM.to_vec()));
    let kept = Mutex::new(None);
    let keep = |password: &str| {
        if let Ok(mut kept) = kept.lock() {
            *kept = Some(password.to_owned());
        }
        Ok(())
    };
    let state = match wire_media_server_admin(&media, &random, recorded, true, &keep).await {
        Ok(password) => {
            wire_request_identity(
                &request_service,
                Protocol("jellyfin".to_owned()),
                &password,
                "http://jellyfin:8096",
                true,
            )
            .await
            .state
        }
        Err(state) => state,
    };
    (state, kept.lock().ok().and_then(|kept| kept.clone()))
}

fn media(startup: Startup, create: Create) -> MediaSetup {
    MediaSetup::new(startup, create)
}

const RANDOM: [u8; 24] = [0x11; 24];

/// A rehearsal against a fresh media server mints no admin password and creates no
/// account, and says what a real pass would do without a value in it.
#[tokio::test]
async fn a_rehearsed_identity_mints_nothing_and_creates_no_account() {
    let (state, minted) = would_identity(
        // A creation that would be refused, so that reaching it at all would show.
        media(Startup::Fresh, Create::Rejects),
        RequestSetup::new(Init::Fresh, Init::Done, Configure::Ok),
        None,
    )
    .await;

    assert_eq!(
        state,
        State::WouldWire {
            yours: None,
            ours: None,
        },
        "a value here is a password that was generated to describe a question"
    );
    assert_eq!(minted, None, "and none was handed back to record");
}

/// And a request service the household already initialised is left alone on both
/// runs, so a rehearsal of that case is the case.
#[tokio::test]
async fn a_rehearsed_identity_leaves_an_initialised_request_service_as_it_is() {
    let (state, minted) = would_identity(
        media(Startup::Completed, Create::Ok),
        RequestSetup::new(Init::Done, Init::Done, Configure::Rejects),
        Some("recorded-already"),
    )
    .await;

    assert_eq!(state, State::AlreadyWired);
    assert_eq!(minted, None);
}

#[tokio::test]
async fn a_media_server_that_is_not_answering_skips_the_identity() {
    let (state, minted) = identity(
        media(Startup::Down, Create::Ok),
        RequestSetup::new(Init::Fresh, Init::Done, Configure::Ok),
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
        RequestSetup::new(Init::Fresh, Init::Done, Configure::Ok),
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
        RequestSetup::new(Init::Fresh, Init::Done, Configure::Ok),
        Some(RANDOM.to_vec()),
        None,
    )
    .await;
    assert_eq!(state, State::Wired);
    assert!(minted.is_some(), "the minted password was recorded");
}

#[tokio::test]
async fn a_rejected_admin_creation_fails_with_the_services_own_words() {
    let (state, minted) = identity(
        media(Startup::Fresh, Create::Rejects),
        RequestSetup::new(Init::Fresh, Init::Done, Configure::Ok),
        Some(RANDOM.to_vec()),
        None,
    )
    .await;
    assert!(matches!(state, State::Failed { .. }), "{state:?}");
    // Recorded before the wizard was given it, so a creation that landed and then
    // answered badly is one lemonfiber still holds the password for. One that did not
    // land is minted again over it by the next run, which finds the wizard waiting.
    assert!(
        minted.is_some(),
        "the password was not recorded before it was set"
    );
}

#[tokio::test]
async fn a_password_that_cannot_be_recorded_is_never_given_to_the_wizard() {
    // The creation would be refused with the service's own words, so a failure naming
    // the record instead is one that stopped before the wizard was asked anything.
    let (state, minted) = identity_keeping(
        media(Startup::Fresh, Create::Rejects),
        RequestSetup::new(Init::Fresh, Init::Done, Configure::Ok),
        Some(RANDOM.to_vec()),
        None,
        false,
    )
    .await;
    assert!(
        matches!(&state, State::Failed { detail } if detail.contains("could not be recorded")),
        "{state:?}"
    );
    assert!(minted.is_none());
}

#[tokio::test]
async fn a_media_server_set_up_outside_lemonfiber_is_skipped() {
    // The wizard has run but lemonfiber recorded no password, so the household set
    // it up: its credential is unknown, and the identity cannot be wired.
    let (state, minted) = identity(
        media(Startup::Completed, Create::Ok),
        RequestSetup::new(Init::Fresh, Init::Done, Configure::Ok),
        Some(RANDOM.to_vec()),
        None,
    )
    .await;
    assert!(matches!(state, State::Skipped { .. }), "{state:?}");
    assert!(minted.is_none());
}

#[tokio::test]
async fn a_recorded_password_wires_an_already_initialised_request_service() {
    // The media server was set up by lemonfiber before (password recorded) and the request
    // service is already initialised: nothing is minted and nothing re-pointed.
    let (state, minted) = identity(
        media(Startup::Completed, Create::Ok),
        RequestSetup::new(Init::Done, Init::Done, Configure::Ok),
        None,
        Some("minted-earlier"),
    )
    .await;
    assert_eq!(state, State::AlreadyWired);
    assert!(minted.is_none(), "an idempotent run mints nothing");
}

#[tokio::test]
async fn a_request_service_that_is_not_answering_still_records_the_minted_password() {
    // The media server's account was created this run, so its password must be recorded
    // even though the request service could not then be reached to finish the wiring.
    let (state, minted) = identity(
        media(Startup::Fresh, Create::Ok),
        RequestSetup::new(Init::Down, Init::Done, Configure::Ok),
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
        RequestSetup::new(Init::Fresh, Init::Done, Configure::Rejects),
        Some(RANDOM.to_vec()),
        None,
    )
    .await;
    assert!(matches!(state, State::Failed { .. }), "{state:?}");
}

#[tokio::test]
async fn a_refused_owner_fails_before_the_request_service_is_read() {
    let (state, _) = identity(
        media(Startup::Fresh, Create::Ok),
        RequestSetup::new(Init::Done, Init::Done, Configure::Ok).refusing_an_owner(),
        Some(RANDOM.to_vec()),
        None,
    )
    .await;
    assert_eq!(
        state,
        State::Failed {
            detail: NO_OWNER.to_owned(),
        }
    );
}

#[tokio::test]
async fn a_sign_in_that_does_not_take_is_a_failure() {
    // The request service accepted the sign-in but still reports itself uninitialised on
    // the read-back, so the write did not land.
    let (state, _) = identity(
        media(Startup::Fresh, Create::Ok),
        RequestSetup::new(Init::Fresh, Init::Fresh, Configure::Ok),
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
        RequestSetup::new(Init::Fresh, Init::Down, Configure::Ok),
        Some(RANDOM.to_vec()),
        None,
    )
    .await;
    assert!(matches!(state, State::Skipped { .. }), "{state:?}");
}

/// A rehearsal against a request service nobody has set up names the media server it
/// would take the household's accounts from, and points it at nothing.
///
/// The other rehearsed case stops at the media server, because a wizard that has not
/// run is an account a real pass would have to create. Here the wizard has run and the
/// password is one lemonfiber already recorded, so the pass gets past that and reaches
/// the second service — which is where the write that costs something lives: signing a
/// fresh request service in creates its owner, and an operator asking what would happen
/// is entitled to know which server those accounts would come from before it does.
/// Both services are scripted to refuse, so reaching either would show as a failure
/// rather than as a rehearsal.
#[tokio::test]
async fn a_rehearsed_identity_names_the_server_a_fresh_request_service_would_answer_to() {
    let (state, minted) = would_identity(
        media(Startup::Completed, Create::Rejects),
        RequestSetup::new(Init::Fresh, Init::Done, Configure::Rejects),
        Some("recorded-already"),
    )
    .await;

    assert_eq!(
        state,
        State::WouldWire {
            yours: None,
            ours: Some("http://jellyfin:8096".to_owned()),
        },
        "the address the household's accounts would come from is not on the report"
    );
    assert_eq!(
        minted, None,
        "a password already recorded was minted a second time"
    );
}
