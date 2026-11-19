use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use lemonfiber_fixtures::ports::Chance;
use lemonfiber_fixtures::scratch::Scratch;

use super::{Holding, Keyring};
use crate::instant::Instant;
use crate::keys::{Kept, Minter, Purpose, Record, Scope, Secret, Used};

fn a_directory(named: &str) -> Scratch {
    Scratch::new(&format!("keyring-{named}"))
}

struct OneKey {
    kept: PathBuf,
    used: PathBuf,
    secret: Secret,
    _dir: Scratch,
}

fn one_key(named: &str, byte: u8, revoked: bool) -> OneKey {
    let dir = a_directory(named);
    let Some(secret) = Secret::mint(&Chance::exactly(Some(vec![byte; 32]))) else {
        unreachable!("thirty-two bytes mint a secret")
    };
    let mut record = Record::minted(
        "home-assistant",
        Scope::Read,
        Purpose::HomeAssistant,
        &secret,
        Instant::from_seconds(1_791_180_000),
        Minter::Operator,
    );
    if revoked {
        record.revoked = Some(Instant::from_seconds(1_791_183_600));
    }
    let kept = dir.join("keys.json");
    assert!(Kept { keys: vec![record] }.keep(&kept).is_ok());
    OneKey {
        kept,
        used: dir.join("keys-used.json"),
        secret,
        _dir: dir,
    }
}

fn noted(key: &OneKey) -> Option<Instant> {
    Used::at(&key.used).at.get("home-assistant").copied()
}

#[test]
fn an_active_key_is_found_by_its_secret() {
    let key = one_key("active", 1, false);
    assert!(
        matches!(Keyring::holding(Some(&key.kept), key.secret.as_str()), Holding::Active(record) if record.name == "home-assistant")
    );
}

#[test]
fn a_revoked_key_is_known_and_refused_rather_than_unknown() {
    let key = one_key("revoked", 2, true);
    assert_eq!(
        Keyring::holding(Some(&key.kept), key.secret.as_str()),
        Holding::Known
    );
}

#[test]
fn a_secret_nobody_minted_is_unknown() {
    let key = one_key("unknown", 3, false);
    let Some(stranger) = Secret::mint(&Chance::exactly(Some(vec![9; 32]))) else {
        unreachable!("thirty-two bytes mint a secret")
    };
    assert_eq!(
        Keyring::holding(Some(&key.kept), stranger.as_str()),
        Holding::Unknown
    );
    assert_eq!(Keyring::holding(None, stranger.as_str()), Holding::Unknown);
}

#[test]
fn a_record_that_does_not_read_refuses_every_key_without_calling_any_a_guess() {
    let key = one_key("damaged", 4, false);
    assert!(std::fs::write(&key.kept, "not keys").is_ok());
    assert_eq!(
        Keyring::holding(Some(&key.kept), key.secret.as_str()),
        Holding::Known
    );
}

#[tokio::test]
async fn a_use_is_written_down_once_a_minute_at_most() {
    let key = one_key("noted", 5, false);
    let keyring = Keyring::default();
    let then = SystemTime::UNIX_EPOCH + Duration::from_secs(1_790_000_000);
    keyring.note(Some(&key.used), "home-assistant", then).await;
    let first = noted(&key);
    assert!(first.is_some());

    keyring
        .note(
            Some(&key.used),
            "home-assistant",
            then + Duration::from_secs(30),
        )
        .await;
    assert_eq!(noted(&key), first);

    keyring
        .note(
            Some(&key.used),
            "home-assistant",
            then + Duration::from_secs(61),
        )
        .await;
    assert_ne!(noted(&key), first);
}

#[tokio::test]
async fn a_keyring_with_nowhere_to_write_a_use_writes_nothing() {
    let dir = a_directory("unwritten");
    Keyring::default()
        .note(None, "home-assistant", SystemTime::now())
        .await;
    assert!(std::fs::read_dir(&dir).is_ok_and(|mut entries| entries.next().is_none()));
}

#[tokio::test]
async fn a_use_at_a_moment_no_date_can_name_is_not_written() {
    let key = one_key("undated", 6, false);
    let before = SystemTime::UNIX_EPOCH - Duration::from_secs(1);
    Keyring::default()
        .note(Some(&key.used), "home-assistant", before)
        .await;
    assert!(Used::at(&key.used).at.is_empty());
}
