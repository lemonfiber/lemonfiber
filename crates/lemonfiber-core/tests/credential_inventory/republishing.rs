//! Handing a service's own key out again.

use crate::{asked, ctx, env_at, recorded, silent, the_service_key, SERVICE_CONFIG};
use lemonfiber_core::app::Asking;
use lemonfiber_fixtures::files::Files;
use lemonfiber_fixtures::http::{Answer, Fake};

/// A Servarr status body, as a healthy service answers `system/status` with.
const SONARR_STATUS: &str = r#"{"instanceName":"Sonarr","version":"4.0.15.2941"}"#;

/// A service that answers to the key it holds has that key handed out again — the
/// repair for one that regenerated it underneath a stack still serving the old copy.
#[tokio::test]
async fn a_service_that_answers_to_its_own_key_has_it_handed_out_again() {
    let stale = format!("{}{}", "0000stale", "keykeykeykey");
    let env = env_at("republish", &[("SONARR_API_KEY", &stale)]);
    let http = Fake::by_path(vec![("/system/status", Answer::reply(200, SONARR_STATUS))]);
    let ctx = ctx(env.clone(), Files::anywhere(SERVICE_CONFIG), http);

    let inventory = asked(
        &ctx,
        Asking::Rotate {
            credential: "Sonarr API key".to_owned(),
        },
    )
    .await;

    let said = format!("{:?}", inventory.rotated.clone().map(|one| one.settled));
    assert!(said.starts_with("Some(Replaced"), "{said}");
    assert!(said.contains("Sonarr"), "{said}");
    assert_eq!(
        inventory.rotated.map(|one| one.consumers.len()),
        Some(2),
        "the service and everything reading it from the environment"
    );
    assert_eq!(recorded(&env, "SONARR_API_KEY"), Some(the_service_key()));
}

/// A service that refuses the key in its own configuration is not one to hand that
/// key to every consumer of it.
#[tokio::test]
async fn a_service_that_refuses_its_own_key_has_nothing_published_for_it() {
    let stale = format!("{}{}", "0000stale", "keykeykeykey");
    let env = env_at("refuseskey", &[("SONARR_API_KEY", &stale)]);
    let http = Fake::by_path(vec![("/system/status", Answer::reply(401, ""))]);
    let ctx = ctx(env.clone(), Files::anywhere(SERVICE_CONFIG), http);

    let inventory = asked(
        &ctx,
        Asking::Rotate {
            credential: "Sonarr API key".to_owned(),
        },
    )
    .await;

    let said = format!("{:?}", inventory.rotated.map(|one| one.settled));
    assert!(said.starts_with("Some(Refused"), "{said}");
    assert_eq!(recorded(&env, "SONARR_API_KEY"), Some(stale));
}

/// A service that answers with something else leaves the key unproven, and its own
/// words are carried through the withholding rule every other sentence goes through.
#[tokio::test]
async fn a_service_that_answers_unusably_leaves_its_key_unproven() {
    let stale = format!("{}{}", "0000stale", "keykeykeykey");
    let env = env_at("unusable", &[("SONARR_API_KEY", &stale)]);
    let http = Fake::by_path(vec![(
        "/system/status",
        Answer::reply(500, "upstream is down"),
    )]);
    let ctx = ctx(env.clone(), Files::anywhere(SERVICE_CONFIG), http);

    let inventory = asked(
        &ctx,
        Asking::Rotate {
            credential: "Sonarr API key".to_owned(),
        },
    )
    .await;

    let said = format!("{:?}", inventory.rotated.map(|one| one.settled));
    assert!(said.starts_with("Some(Unproven"), "{said}");
    assert!(said.contains("still in force"), "{said}");
    assert_eq!(recorded(&env, "SONARR_API_KEY"), Some(stale));
}

/// A service that has written no key has none to hand out.
#[tokio::test]
async fn a_service_that_has_written_no_key_has_none_to_hand_out() {
    let env = env_at("unwritten", &[]);
    let ctx = ctx(env, Files::empty(), silent());

    let inventory = asked(
        &ctx,
        Asking::Rotate {
            credential: "Sonarr API key".to_owned(),
        },
    )
    .await;

    let said = format!("{:?}", inventory.rotated.map(|one| one.settled));
    assert!(said.starts_with("Some(Unproven"), "{said}");
    assert!(said.contains("has not written an API key"), "{said}");
}

/// A service key lemonfiber cannot prove by asking the service who it is is one it
/// will not hand out unproven — seeding already does that, and the answer says so.
#[tokio::test]
async fn a_key_that_cannot_be_proven_by_identity_points_at_the_seeding_instead() {
    let env = env_at("notservarr", &[]);
    let ctx = ctx(env, Files::anywhere(SERVICE_CONFIG), silent());

    let inventory = asked(
        &ctx,
        Asking::Rotate {
            credential: "SABNZBD_API_KEY".to_owned(),
        },
    )
    .await;

    let said = format!("{:?}", inventory.rotated.map(|one| one.settled));
    assert!(said.starts_with("Some(Elsewhere"), "{said}");
    assert!(said.contains("lemonfiber seed"), "{said}");
}

/// A rehearsed republish reads no key out of the service's own file and asks the
/// service nothing.
///
/// Both halves are above the line this stops at, and each costs something on its own.
/// Reading the key would put a credential into this run's memory to describe a rotation
/// nobody asked it to make, and asking the service to identify itself with it is an
/// authentication attempt in somebody's log that nobody asked for either. So the answer
/// names the place and the steps, and the key that was already recorded is still the
/// one recorded.
#[tokio::test]
async fn a_rehearsed_republish_reads_no_key_and_asks_the_service_nothing() {
    let stale = format!("{}{}", "0000stale", "keykeykeykey");
    let env = env_at("rehearsed-republish", &[("SONARR_API_KEY", &stale)]);
    let http = Fake::by_path(vec![("/system/status", Answer::reply(200, SONARR_STATUS))]);
    let ctx = ctx(env.clone(), Files::anywhere(SERVICE_CONFIG), http.clone()).rehearsing();

    let inventory = asked(
        &ctx,
        Asking::Rotate {
            credential: "Sonarr API key".to_owned(),
        },
    )
    .await;

    let said = format!("{:?}", inventory.rotated.map(|one| one.settled));
    assert!(said.starts_with("Some(Rehearsed"), "{said}");
    assert!(said.contains("Nothing was read out"), "{said}");
    assert!(!said.contains(&the_service_key()), "{said}");
    assert_eq!(recorded(&env, "SONARR_API_KEY"), Some(stale));
    let reached = http.requests();
    assert!(
        reached.is_empty(),
        "a rehearsal asked the service to identify itself: {reached:?}"
    );
}
