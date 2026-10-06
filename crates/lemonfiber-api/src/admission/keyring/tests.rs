use std::time::{Duration, SystemTime};

use lemonfiber_core::keys::{Kept, Minter, Purpose, Record, Scope, Secret, Used};
use lemonfiber_fixtures::ports::Chance;

use super::{Holding, Keyring};

/// A directory of this case's own, emptied first.
fn a_directory(named: &str) -> std::path::PathBuf {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("keyring-{named}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// A keyring over one key minted from `byte`, revoked where `revoked`.
fn one_key(named: &str, byte: u8, revoked: bool) -> (Keyring, Secret) {
    let dir = a_directory(named);
    let Some(secret) = Secret::mint(&Chance::exactly(Some(vec![byte; 32]))) else {
        unreachable!("thirty-two bytes mint a secret")
    };
    let mut record = Record::minted(
        "home-assistant",
        Scope::Read,
        Purpose::HomeAssistant,
        &secret,
        "2026-10-05T06:00:00".to_owned(),
        Minter::Operator,
    );
    if revoked {
        record.revoked = Some("2026-10-05T07:00:00".to_owned());
    }
    let kept = dir.join("keys.json");
    assert!(Kept { keys: vec![record] }.keep(&kept).is_ok());
    (
        Keyring::at(Some(kept), Some(dir.join("keys-used.json"))),
        secret,
    )
}

#[test]
fn an_active_key_is_found_by_its_secret() {
    let (keyring, secret) = one_key("active", 1, false);
    assert!(
        matches!(keyring.holding(secret.as_str()), Holding::Active(record) if record.name == "home-assistant")
    );
}

#[test]
fn a_revoked_key_is_known_and_refused_rather_than_unknown() {
    let (keyring, secret) = one_key("revoked", 2, true);
    assert_eq!(keyring.holding(secret.as_str()), Holding::Known);
}

#[test]
fn a_secret_nobody_minted_is_unknown() {
    let (keyring, _) = one_key("unknown", 3, false);
    let Some(stranger) = Secret::mint(&Chance::exactly(Some(vec![9; 32]))) else {
        unreachable!("thirty-two bytes mint a secret")
    };
    assert_eq!(keyring.holding(stranger.as_str()), Holding::Unknown);
    assert_eq!(
        Keyring::default().holding(stranger.as_str()),
        Holding::Unknown
    );
}

#[test]
fn a_record_that_does_not_read_refuses_every_key_without_calling_any_a_guess() {
    let (keyring, secret) = one_key("damaged", 4, false);
    if let Some(path) = keyring.kept.as_deref() {
        assert!(std::fs::write(path, "not keys").is_ok());
    }
    assert_eq!(keyring.holding(secret.as_str()), Holding::Known);
}

#[tokio::test]
async fn a_use_is_written_down_once_a_minute_at_most() {
    let (keyring, _) = one_key("noted", 5, false);
    let then = SystemTime::UNIX_EPOCH + Duration::from_secs(1_790_000_000);
    keyring.note("home-assistant", then).await;
    let used = keyring.used.as_deref().map(Used::at).unwrap_or_default();
    let first = used.at.get("home-assistant").cloned();
    assert!(first.is_some());

    keyring
        .note("home-assistant", then + Duration::from_secs(30))
        .await;
    let used = keyring.used.as_deref().map(Used::at).unwrap_or_default();
    assert_eq!(used.at.get("home-assistant").cloned(), first);

    keyring
        .note("home-assistant", then + Duration::from_secs(61))
        .await;
    let used = keyring.used.as_deref().map(Used::at).unwrap_or_default();
    assert_ne!(used.at.get("home-assistant").cloned(), first);
}

#[tokio::test]
async fn a_keyring_with_nowhere_to_write_a_use_writes_nothing() {
    let dir = a_directory("unwritten");
    let keyring = Keyring::at(Some(dir.join("keys.json")), None);
    keyring.note("home-assistant", SystemTime::now()).await;
    assert!(std::fs::read_dir(&dir).is_ok_and(|mut entries| entries.next().is_none()));
}

#[tokio::test]
async fn a_use_at_a_moment_no_date_can_name_is_not_written() {
    let (keyring, _) = one_key("undated", 6, false);
    let before = SystemTime::UNIX_EPOCH - Duration::from_secs(1);
    keyring.note("home-assistant", before).await;
    let used = keyring.used.as_deref().map(Used::at).unwrap_or_default();
    assert!(used.at.is_empty());
}
