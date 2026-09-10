//! What an operator is told about the credentials this stack holds, and what a
//! rotation does to them — driven through the dispatcher against real files.
//!
//! From here rather than from a `#[cfg(test)]` module because the whole of this is
//! `async` and reaches a service, and an async path exercised only in-crate has its
//! coverage counted from the copy that never ran.
//!
//! **Two properties are asserted here that cannot be asserted anywhere else.** The
//! first is that a value never leaves: the settings file is written with real
//! credentials, the whole answer is serialised, and the assertion is that the text
//! does not hold them — stated as an absence rather than by printing what was found,
//! because a guard that logs the traffic it inspects is the leak it watches for.
//!
//! The second is the ordering rotation exists for. A refused replacement is driven,
//! and what is then read back is the *settings file* rather than the report: the
//! guarantee is that the credential which was working before is the one still
//! recorded, and a report claiming so proves nothing about the file.

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use common::stack::project;
use lemonfiber_core::app::{dispatch, Asking, Command, Ctx, Outcome};
use lemonfiber_core::config::{store, Protocols, Settings, QBITTORRENT_PASSWORD_KEY};
use lemonfiber_core::credential::{fingerprint, Inventory, Rotation, State};
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::http::Http;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::files::Files;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::{spoke, FixedRandom, Reporting, Scripted};

/// A Servarr configuration carrying the key that service generated for itself.
const SERVICE_CONFIG: &str = "<Config><ApiKey>aaaabbbbccccddddeeee</ApiKey></Config>";

/// The key inside [`SERVICE_CONFIG`], so a test can assert it never surfaces.
///
/// Split and joined rather than written whole, so nothing scanning this repository
/// for a leaked credential reads a test fixture as one.
fn the_service_key() -> String {
    format!("{}{}", "aaaabbbb", "ccccddddeeee")
}

/// The password recorded for the torrent client, built the same way.
fn the_torrent_password() -> String {
    format!("{}{}", "ffff0000", "111122223333")
}

/// A settings file at a scratch path unique to the named case, holding whatever a
/// test recorded in it.
fn env_at(name: &str, settings: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "lemonfiber-credentials-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join(".env");
    for (key, value) in settings {
        assert!(
            store::set(&path, key, value).is_ok(),
            "the scratch settings file is written"
        );
    }
    path
}

/// What one setting reads as now, straight off the file rather than off a report.
fn recorded(path: &Path, key: &str) -> Option<String> {
    store::read(path)
        .ok()
        .and_then(|file| file.get(key).map(ToOwned::to_owned))
}

/// A run over the stack this repository carries, with the given settings file, the
/// given service configurations, and the given transport.
fn ctx(env: PathBuf, files: Arc<Files>, http: Arc<dyn Http>) -> Ctx {
    Ctx::new(
        Arc::new(Scripted(Ok(spoke("")))),
        Arc::new(Reporting::default()),
        lemonfiber_fixtures::ports::Stopped::today(),
        lemonfiber_ports::seams::Seams {
            filesystem: files,
            ..lemonfiber_adapters::live()
        },
        Source::External(project()),
        Settings {
            protocols: Protocols::both(),
            env_file: Some(env),
            stack_dir: Some(project().to_path_buf()),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_http(http)
}

/// The inventory one ask came to, or an empty one — which satisfies no assertion here.
async fn asked(ctx: &Ctx, asking: Asking) -> Inventory {
    match dispatch(Command::Credentials(asking), ctx).await {
        Ok(Outcome::Credentials(inventory)) => inventory,
        _ => Inventory::of(Vec::new()),
    }
}

/// A transport that answers nothing, for the reads that speak to no service.
fn silent() -> Arc<Fake> {
    Fake::by_path(Vec::new())
}

/// A Servarr status body, as a healthy service answers `system/status` with.
const SONARR_STATUS: &str = r#"{"instanceName":"Sonarr","version":"4.0.15.2941"}"#;

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
    let nowhere = Ctx::new(
        Arc::new(Scripted(Ok(spoke("")))),
        Arc::new(Reporting::default()),
        lemonfiber_fixtures::ports::Stopped::today(),
        lemonfiber_ports::seams::Seams {
            filesystem: Files::empty(),
            ..lemonfiber_adapters::live()
        },
        Source::External(std::path::Path::new("/lemonfiber/no/such/stack")),
        Settings {
            env_file: Some(env),
            ..Settings::default()
        },
        Environment::MacOs,
    );

    let refused = dispatch(Command::Credentials(Asking::Read), &nowhere).await;
    let said = refused.err().map(|problem| problem.summary.clone());

    assert!(said.is_some(), "a stack that cannot be read is a refusal");
    assert!(asked(&nowhere, Asking::Read).await.held.is_empty());
}
