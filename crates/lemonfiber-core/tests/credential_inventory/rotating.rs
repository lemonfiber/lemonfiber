//! Seeing a credential once, and replacing one.

use crate::{asked, beside, ctx, env_at, recorded, silent, the_torrent_password};
use lemonfiber_core::app::Asking;
use lemonfiber_core::config::QBITTORRENT_PASSWORD_KEY;
use lemonfiber_core::credential::Rotation;
use lemonfiber_fixtures::files::Files;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::FixedRandom;
use std::sync::Arc;

/// The heart of it: a replacement the service refuses leaves the credential that was
/// working before recorded exactly as it was.
#[tokio::test]
async fn a_refused_rotation_leaves_the_recorded_password_exactly_as_it_was() {
    let password = the_torrent_password();
    let env = env_at("refused", &[(QBITTORRENT_PASSWORD_KEY, &password)]);
    // The client authenticates with the password it holds before it sets anything.
    // Refused there, nothing is set and nothing is written.
    let http = Fake::by_path(vec![("/auth/login", Answer::reply(200, "Fails."))]);
    let ctx = ctx(env.clone(), Files::empty(), http);

    let inventory = asked(
        &ctx,
        Asking::Rotate {
            credential: "qBittorrent web UI password".to_owned(),
        },
    )
    .await;

    let rotated = inventory.rotated.clone();
    assert!(rotated.is_some(), "a rotation was asked for and answered");
    let kept = rotated.as_ref().is_some_and(Rotation::kept_the_existing);
    assert!(
        kept,
        "the credential that was working is still the one in force"
    );
    let refused = format!("{:?}", rotated.map(|one| one.settled));
    assert!(refused.starts_with("Some(Refused"), "{refused}");
    // The file, not the report. A report claiming the old value survived proves
    // nothing about whether it did.
    assert_eq!(recorded(&env, QBITTORRENT_PASSWORD_KEY), Some(password));
}

#[tokio::test]
async fn a_credential_the_operators_provider_issued_is_not_replaced_here_and_says_where() {
    let key = format!("{}{}", "1111aaaa", "2222bbbb3333");
    let env = env_at("elsewhere", &[("INDEXER_APIKEY", &key)]);
    let ctx = ctx(env.clone(), Files::empty(), silent());

    let inventory = asked(
        &ctx,
        Asking::Rotate {
            credential: "Indexer API key".to_owned(),
        },
    )
    .await;

    let said = format!("{:?}", inventory.rotated.map(|one| one.settled));
    assert!(said.starts_with("Some(Elsewhere"), "{said}");
    assert!(said.contains("indexer's own account page"), "{said}");
    assert_eq!(recorded(&env, "INDEXER_APIKEY"), Some(key));
}

#[tokio::test]
async fn a_name_nothing_answers_to_changes_nothing_and_says_what_would_have() {
    let password = the_torrent_password();
    let env = env_at("unknown", &[(QBITTORRENT_PASSWORD_KEY, &password)]);
    let ctx = ctx(env.clone(), Files::empty(), silent());

    let inventory = asked(
        &ctx,
        Asking::Rotate {
            credential: "the wifi password".to_owned(),
        },
    )
    .await;

    assert!(inventory.held.len() >= 7, "{}", inventory.held.len());
    let said = format!("{:?}", inventory.rotated.map(|one| one.settled));
    assert!(said.starts_with("Some(Unknown"), "{said}");
    assert!(said.contains("qBittorrent web UI password"), "{said}");
    assert!(said.contains("Indexer API key"), "{said}");
    assert_eq!(recorded(&env, QBITTORRENT_PASSWORD_KEY), Some(password));
}

#[tokio::test]
async fn asking_to_see_a_credential_once_prints_the_warning_and_not_the_value() {
    let password = the_torrent_password();
    let env = env_at("unconfirmed", &[(QBITTORRENT_PASSWORD_KEY, &password)]);
    let ctx = ctx(env, Files::empty(), silent());

    let inventory = asked(
        &ctx,
        Asking::Reveal {
            credential: "qBittorrent web UI password".to_owned(),
            confirmed: false,
        },
    )
    .await;
    let sent = serde_json::to_string(&inventory).unwrap_or_default();

    let revealed = inventory.revealed.clone();
    assert!(revealed.is_some_and(|one| one.value.is_none()));
    assert!(sent.contains("scrollback"), "the warning is said");
    assert!(!sent.contains(&password), "and the value is not");
}

#[tokio::test]
async fn asking_again_prints_the_value_and_the_warning_stays_beside_it() {
    let password = the_torrent_password();
    let env = env_at("confirmed", &[(QBITTORRENT_PASSWORD_KEY, &password)]);
    let ctx = ctx(env, Files::empty(), silent());

    let inventory = asked(
        &ctx,
        Asking::Reveal {
            credential: "qbittorrent".to_owned(),
            confirmed: true,
        },
    )
    .await;

    let revealed = inventory.revealed.clone();
    assert!(revealed.is_some(), "a reveal was asked for and answered");
    assert_eq!(
        revealed.as_ref().and_then(|one| one.value.clone()),
        Some(password)
    );
    let warned = revealed.is_some_and(|one| one.warning.contains("Clear your scrollback"));
    assert!(warned, "the warning stays beside the value");
    // And the listing beside it still holds none of it.
    let listed = serde_json::to_string(&inventory.held).unwrap_or_default();
    assert!(!listed.contains("Clear your scrollback"));
}

/// A landed replacement: qBittorrent takes the new password and signs in with it, and
/// only then is anything written.
#[tokio::test]
async fn a_proven_replacement_is_recorded_and_every_consumer_is_accounted_for() {
    let password = the_torrent_password();
    let env = env_at("landed", &[(QBITTORRENT_PASSWORD_KEY, &password)]);
    // The client signs in with the password it holds, sets the new one, and signs in
    // again with that — three calls, all of which this answers.
    let http = Fake::by_path(vec![
        ("/auth/login", Answer::reply(200, "Ok.")),
        ("/app/setPreferences", Answer::reply(200, "")),
    ]);
    let ctx = ctx(env.clone(), Files::empty(), http);

    let inventory = asked(
        &ctx,
        Asking::Rotate {
            credential: "qBittorrent web UI password".to_owned(),
        },
    )
    .await;

    let rotated = inventory.rotated.clone();
    assert!(rotated.is_some(), "a rotation was asked for and answered");
    let landed = rotated.as_ref().is_some_and(|one| !one.kept_the_existing());
    assert!(landed, "the replacement is the value in force");
    // Every consumer is named, including the ones a restart still has to reach.
    assert_eq!(rotated.map(|one| one.consumers.len()), Some(4));
    // The record moved, and what it moved to is not what it was.
    let now = recorded(&env, QBITTORRENT_PASSWORD_KEY);
    assert!(now.is_some(), "a password is still recorded");
    assert_ne!(now, Some(password), "and it is not the one it was");
}

/// A rotation the service could not be reached for leaves the existing password in
/// force, and says which it kept.
///
/// The distinction the whole feature turns on: a replacement that was never proven is
/// not written down, so what was working is still what works. A refusal is one thing
/// and a service that did not answer is another, and neither may end with the operator
/// holding a password nothing accepts.
#[tokio::test]
async fn a_rotation_that_could_not_reach_the_service_keeps_the_existing_password() {
    let password = the_torrent_password();
    let env = env_at("unreachable", &[(QBITTORRENT_PASSWORD_KEY, &password)]);
    let ctx = ctx(env.clone(), Files::empty(), silent());

    let inventory = asked(
        &ctx,
        Asking::Rotate {
            credential: "qBittorrent web UI password".to_owned(),
        },
    )
    .await;

    let rotated = inventory.rotated.clone();
    let kept = rotated.as_ref().is_some_and(Rotation::kept_the_existing);
    assert!(kept, "{rotated:?}");
    // Read back from the file rather than from the report: the guarantee is about what
    // is in force, not about what was said.
    assert_eq!(recorded(&env, QBITTORRENT_PASSWORD_KEY), Some(password));
}

/// Without randomness there is nothing to generate, and a guessable password on the
/// client the forwarded port authenticates to is worse than the one in force.
#[tokio::test]
async fn without_randomness_nothing_is_generated_and_nothing_is_written() {
    let password = the_torrent_password();
    let env = env_at("unlucky", &[(QBITTORRENT_PASSWORD_KEY, &password)]);
    let ctx = ctx(env.clone(), Files::empty(), silent()).with_random(Arc::new(FixedRandom(None)));

    let inventory = asked(
        &ctx,
        Asking::Rotate {
            credential: "qBittorrent web UI password".to_owned(),
        },
    )
    .await;

    let said = format!("{:?}", inventory.rotated.map(|one| one.settled));
    assert!(said.starts_with("Some(Unproven"), "{said}");
    assert!(said.contains("randomness"), "{said}");
    assert_eq!(recorded(&env, QBITTORRENT_PASSWORD_KEY), Some(password));
}

/// Nothing recorded and nothing to authenticate with is one situation, not two.
#[tokio::test]
async fn with_no_password_recorded_there_is_nothing_to_change_it_with() {
    let env = env_at("nopassword", &[]);
    let ctx = ctx(env, Files::empty(), silent());

    let inventory = asked(
        &ctx,
        Asking::Rotate {
            credential: "qBittorrent web UI password".to_owned(),
        },
    )
    .await;

    let said = format!("{:?}", inventory.rotated.map(|one| one.settled));
    assert!(said.starts_with("Some(Unproven"), "{said}");
    assert!(said.contains("lemonfiber seed"), "{said}");
}

/// A rehearsed rotation of the credential lemonfiber mints generates nothing and asks
/// the client for nothing, and still names where the value lives.
///
/// It stops one line above the generating, and that line is the point. A password
/// minted to describe a rotation is a secret that exists because somebody asked a
/// question, and it would then have to be kept — put where the real one goes, which is
/// the write — or thrown away, which is worse, because a thrown-away one may be the one
/// the client has already taken. The client is scripted to accept a sign-in on purpose,
/// so a run that reached it would land a replacement and this would catch it.
#[tokio::test]
async fn a_rehearsed_rotation_mints_nothing_and_asks_the_client_for_nothing() {
    let password = the_torrent_password();
    let env = env_at("rehearsed-mint", &[(QBITTORRENT_PASSWORD_KEY, &password)]);
    let http = Fake::by_path(vec![("/auth/login", Answer::reply(200, "Ok."))]);
    let ctx = ctx(env.clone(), Files::empty(), http.clone()).rehearsing();

    let inventory = asked(
        &ctx,
        Asking::Rotate {
            credential: "qBittorrent web UI password".to_owned(),
        },
    )
    .await;

    let said = format!("{:?}", inventory.rotated.map(|one| one.settled));
    assert!(said.starts_with("Some(Rehearsed"), "{said}");
    assert!(said.contains("Nothing was generated"), "{said}");
    // The file, not the report: a report claiming the recorded password survived proves
    // nothing about whether it did.
    assert_eq!(recorded(&env, QBITTORRENT_PASSWORD_KEY), Some(password));
    let reached = http.requests();
    assert!(
        reached.is_empty(),
        "a rehearsal signed in to the client: {reached:?}"
    );
}
