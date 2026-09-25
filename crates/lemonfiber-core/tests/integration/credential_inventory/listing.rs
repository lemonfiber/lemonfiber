//! What the inventory lists, and what it never prints.

use super::{asked, ctx, env_at, silent, the_service_key, the_torrent_password, SERVICE_CONFIG};
use lemonfiber_core::app::{dispatch, Asking, Command};
use lemonfiber_core::config::{Settings, QBITTORRENT_PASSWORD_KEY};
use lemonfiber_core::credential::{fingerprint, State};
use lemonfiber_fixtures::files::Files;
use lemonfiber_fixtures::support::Reporting;
use std::sync::Arc;

/// A credential kept without being proven reads as stale, not active, and says why in
/// the operator's own terms: they chose it, nothing has checked it since, and nothing
/// here takes it away from them.
#[tokio::test]
async fn a_credential_kept_without_being_proven_is_stale_and_says_so() {
    let env = env_at(
        "unproven",
        &[("INDEXER_APIKEY", "the-key"), ("INDEXER_VALIDATED", "off")],
    );
    let inventory = asked(&ctx(env, Files::empty(), silent()), Asking::Read).await;
    let said = format!(
        "{:?}",
        inventory
            .held
            .iter()
            .find(|one| one.name == "Indexer API key")
    );
    assert!(said.contains("Stale"), "{said}");
    assert!(said.contains("kept without being proven"), "{said}");
    // The advisory is the whole point: an operator who chose this is told it stands,
    // not that something has quietly undone it.
    assert!(said.contains("Nothing here expires it"), "{said}");

    // And the other side of the same flag: one that was proven is active, with nothing
    // to advise about. The two arms are one decision, so neither is read alone.
    let proven = env_at(
        "proven",
        &[("INDEXER_APIKEY", "the-key"), ("INDEXER_VALIDATED", "on")],
    );
    let inventory = asked(&ctx(proven, Files::empty(), silent()), Asking::Read).await;
    let said = format!(
        "{:?}",
        inventory
            .held
            .iter()
            .find(|one| one.name == "Indexer API key")
    );
    assert!(said.contains("Active"), "{said}");
    assert!(!said.contains("kept without being proven"), "{said}");
}

#[tokio::test]
async fn the_inventory_names_every_credential_the_operator_supplies_and_every_one_it_mints() {
    let env = env_at("declared", &[]);
    let inventory = asked(&ctx(env, Files::empty(), silent()), Asking::Read).await;
    let named: Vec<&str> = inventory.held.iter().map(|one| one.name.as_str()).collect();

    assert!(named.len() >= 7, "{named:?}");
    for expected in [
        "VPN private key",
        "Usenet provider password",
        "Indexer API key",
        "qBittorrent web UI password",
        "Jellyfin administrator password",
    ] {
        assert!(named.contains(&expected), "{expected} is listed: {named:?}");
    }
}

#[tokio::test]
async fn every_line_says_what_uses_it_where_it_is_and_where_it_stands() {
    let env = env_at("columns", &[]);
    let inventory = asked(&ctx(env, Files::empty(), silent()), Asking::Read).await;

    assert!(!inventory.held.is_empty());
    for held in &inventory.held {
        assert!(!held.consumers.is_empty(), "{} has consumers", held.name);
        assert!(!held.location.is_empty(), "{} says where", held.name);
        assert!(!held.setting.is_empty(), "{} names its setting", held.name);
        assert!(!held.state.as_str().is_empty(), "{} has a state", held.name);
    }
}

/// The property the whole shape exists for, asserted over the wire form.
#[tokio::test]
async fn no_recorded_value_appears_anywhere_in_the_answer() {
    let password = the_torrent_password();
    let indexer = format!("{}{}", "9999aaaa", "bbbbccccdddd");
    let env = env_at(
        "secrecy",
        &[
            (QBITTORRENT_PASSWORD_KEY, &password),
            ("INDEXER_APIKEY", &indexer),
        ],
    );
    let ctx = ctx(env, Files::anywhere(SERVICE_CONFIG), silent());

    let inventory = asked(&ctx, Asking::Read).await;
    let sent = serde_json::to_string(&inventory).unwrap_or_default();

    // Asserted before the absences, because an empty answer holds no credential
    // either and would satisfy every one of them.
    assert!(inventory.held.len() >= 7, "{}", inventory.held.len());
    assert!(
        sent.len() > 500,
        "the answer was serialised: {}",
        sent.len()
    );
    assert!(
        !sent.contains(&password),
        "the torrent password is not in it"
    );
    assert!(!sent.contains(&indexer), "the indexer key is not in it");
    assert!(
        !sent.contains(&the_service_key()),
        "no service key is in it"
    );
    // And the likeness that stands in for each is, so the absence above is not
    // simply an answer that said nothing about them.
    assert!(sent.contains(&fingerprint(&password)), "a likeness is");
}

#[tokio::test]
async fn what_the_storage_protects_against_is_stated_with_what_it_does_not() {
    let env = env_at("honesty", &[]);
    let inventory = asked(&ctx(env, Files::empty(), silent()), Asking::Read).await;

    assert!(inventory.protection.summary.contains("not encrypted"));
    assert!(!inventory.protection.against.is_empty());
    assert!(inventory
        .protection
        .not_against
        .join(" ")
        .contains("malware"));
}

/// A service that regenerated its key underneath a stack still handing out the old
/// one, which is the fault the comparison exists to find.
#[tokio::test]
async fn a_service_key_that_no_longer_matches_the_published_copy_is_reported_as_invalid() {
    let stale = format!("{}{}", "0000stale", "keykeykeykey");
    let env = env_at("regenerated", &[("SONARR_API_KEY", &stale)]);
    let ctx = ctx(env, Files::anywhere(SERVICE_CONFIG), silent());

    let inventory = asked(&ctx, Asking::Read).await;
    let sonarr = inventory
        .held
        .iter()
        .find(|one| one.name == "Sonarr API key");

    assert!(
        sonarr.is_some(),
        "the stack carries Sonarr, so its key is listed"
    );
    assert_eq!(sonarr.map(|one| one.state), Some(State::Invalid));
    let said = sonarr
        .and_then(|one| one.advisory.clone())
        .unwrap_or_default();
    assert!(said.contains("regenerated"), "{said}");
    assert!(said.contains(&fingerprint(&stale)), "{said}");
    assert!(said.contains(&fingerprint(&the_service_key())), "{said}");
    assert!(!said.contains(&stale), "{said}");
    assert!(!said.contains(&the_service_key()), "{said}");
}

/// A reveal that names nothing is answered as a reveal, not as a replacement nobody
/// asked for.
#[tokio::test]
async fn asking_to_see_something_this_stack_does_not_hold_says_what_it_does() {
    let env = env_at("noreveal", &[]);
    let ctx = ctx(env, Files::empty(), silent());

    let inventory = asked(
        &ctx,
        Asking::Reveal {
            credential: "the wifi password".to_owned(),
            confirmed: true,
        },
    )
    .await;

    assert!(inventory.rotated.is_none(), "nothing was being replaced");
    let said = inventory
        .revealed
        .map(|one| one.warning)
        .unwrap_or_default();
    assert!(said.contains("Nothing here is called"), "{said}");
    assert!(said.contains("qBittorrent web UI password"), "{said}");
}

/// A stack that cannot be read refuses rather than reporting that this machine holds
/// no credentials, which would be a claim rather than a gap.
#[tokio::test]
async fn a_stack_that_cannot_be_read_refuses_rather_than_reporting_none() {
    let env = env_at("nostack", &[]);
    let nowhere = lemonfiber_testing::a_context()
        .engine(Arc::new(Reporting::default()))
        .filesystem(Files::empty())
        .over(lemonfiber_testing::nowhere())
        .settings(Settings {
            env_file: Some(env),
            ..Settings::default()
        })
        .build();

    let refused = dispatch(Command::Credentials(Asking::Read), &nowhere).await;
    let said = refused.err().map(|problem| problem.summary.clone());

    assert!(said.is_some(), "a stack that cannot be read is a refusal");
    assert!(asked(&nowhere, Asking::Read).await.held.is_empty());
}
